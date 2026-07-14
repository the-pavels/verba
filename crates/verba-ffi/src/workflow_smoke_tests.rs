use std::{
    collections::VecDeque,
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant},
};

use verba_core::{
    capture::{CapturedText, TextCapture},
    coordinator::{CancellationToken, PresentationUpdate, ResultPresenter, ShortcutCoordinator},
    presentation::TextAction,
    proofreading::{
        Proofreader, ProofreaderError, ProofreaderResponse, ProofreadingCorrection,
        ProofreadingRequest,
    },
    shortcut::{
        ShortcutConfiguration, ShortcutEventHandler, ShortcutRegistry, ShortcutRegistryError,
    },
    translation::{
        LanguageIdentifier, TranslationPreferences, TranslationSettingsStore,
        TranslationSettingsStoreError,
    },
};

use crate::{
    LanguagePairViewModel, NativeTranslationError, NativeTranslationRequest,
    NativeTranslationResponse, NativeTranslator, PresentationAction, PresentationViewModel,
    processor::ApplicationProcessor,
};

const WAIT_TIMEOUT: Duration = Duration::from_secs(2);

struct QueueCapture {
    selections: Mutex<VecDeque<CapturedText>>,
}

impl QueueCapture {
    fn new(selections: impl IntoIterator<Item = &'static str>) -> Arc<Self> {
        Arc::new(Self {
            selections: Mutex::new(
                selections
                    .into_iter()
                    .map(|text| CapturedText::new(text).unwrap())
                    .collect(),
            ),
        })
    }
}

impl TextCapture for QueueCapture {
    fn capture(&self) -> Result<CapturedText, verba_core::capture::CaptureFailure> {
        Ok(self
            .selections
            .lock()
            .unwrap()
            .pop_front()
            .expect("the workflow should have a queued selection"))
    }
}

#[derive(Default)]
struct PopupRecorder {
    updates: Mutex<Vec<(u64, PresentationViewModel)>>,
    changed: Condvar,
}

impl PopupRecorder {
    fn wait_until(
        &self,
        predicate: impl Fn(&[(u64, PresentationViewModel)]) -> bool,
    ) -> Vec<(u64, PresentationViewModel)> {
        let deadline = Instant::now() + WAIT_TIMEOUT;
        let mut updates = self.updates.lock().unwrap();

        while !predicate(&updates) {
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .expect("timed out waiting for a popup update");
            let (next_updates, result) = self.changed.wait_timeout(updates, remaining).unwrap();
            updates = next_updates;
            assert!(!result.timed_out(), "timed out waiting for a popup update");
        }

        updates.clone()
    }

    fn wait_for_count(&self, count: usize) -> Vec<(u64, PresentationViewModel)> {
        self.wait_until(|updates| updates.len() >= count)
    }
}

impl ResultPresenter for PopupRecorder {
    fn present(&self, update: PresentationUpdate) {
        self.updates
            .lock()
            .unwrap()
            .push((update.request_id.value(), update.state.into()));
        self.changed.notify_all();
    }
}

#[derive(Default)]
struct WorkflowShortcutRegistry {
    handler: Option<Arc<dyn ShortcutEventHandler>>,
    registrations: usize,
    unregistrations: usize,
}

impl WorkflowShortcutRegistry {
    fn trigger(&self, action: TextAction) {
        self.handler
            .as_ref()
            .expect("shortcuts should be registered")
            .on_shortcut(action);
    }
}

impl ShortcutRegistry for WorkflowShortcutRegistry {
    fn register(
        &mut self,
        _shortcuts: &ShortcutConfiguration,
        event_handler: Arc<dyn ShortcutEventHandler>,
    ) -> Result<(), ShortcutRegistryError> {
        self.handler = Some(event_handler);
        self.registrations += 1;
        Ok(())
    }

    fn unregister_all(&mut self) -> Result<(), ShortcutRegistryError> {
        self.handler = None;
        self.unregistrations += 1;
        Ok(())
    }
}

struct RecordingNativeTranslator {
    responses: Mutex<VecDeque<Result<NativeTranslationResponse, NativeTranslationError>>>,
    requests: Mutex<Vec<NativeTranslationRequest>>,
}

impl RecordingNativeTranslator {
    fn new(responses: impl IntoIterator<Item = &'static str>) -> Arc<Self> {
        Arc::new(Self {
            responses: Mutex::new(
                responses
                    .into_iter()
                    .map(|translated_text| {
                        Ok(NativeTranslationResponse {
                            source_language_identifier: "de".to_owned(),
                            translated_text: translated_text.to_owned(),
                        })
                    })
                    .collect(),
            ),
            requests: Mutex::new(Vec::new()),
        })
    }
}

#[async_trait::async_trait]
impl NativeTranslator for RecordingNativeTranslator {
    async fn translate(
        &self,
        request: NativeTranslationRequest,
    ) -> Result<NativeTranslationResponse, NativeTranslationError> {
        self.requests.lock().unwrap().push(request);
        self.responses
            .lock()
            .unwrap()
            .pop_front()
            .expect("the workflow should have a queued translation response")
    }
}

struct BlockingNativeTranslator {
    requests: Mutex<Vec<NativeTranslationRequest>>,
    started: Mutex<bool>,
    dropped: Mutex<bool>,
    started_changed: Condvar,
    dropped_changed: Condvar,
}

impl BlockingNativeTranslator {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            requests: Mutex::new(Vec::new()),
            started: Mutex::new(false),
            dropped: Mutex::new(false),
            started_changed: Condvar::new(),
            dropped_changed: Condvar::new(),
        })
    }

    fn wait_until_started(&self) {
        let started = self.started.lock().unwrap();
        let (started, result) = self
            .started_changed
            .wait_timeout_while(started, WAIT_TIMEOUT, |started| !*started)
            .unwrap();
        assert!(*started && !result.timed_out(), "translation did not start");
    }

    fn wait_until_dropped(&self) {
        wait_for_signal(
            &self.dropped,
            &self.dropped_changed,
            "translation did not stop",
        );
    }
}

#[async_trait::async_trait]
impl NativeTranslator for BlockingNativeTranslator {
    async fn translate(
        &self,
        request: NativeTranslationRequest,
    ) -> Result<NativeTranslationResponse, NativeTranslationError> {
        self.requests.lock().unwrap().push(request);
        *self.started.lock().unwrap() = true;
        self.started_changed.notify_all();
        let _drop_signal = DropSignal {
            dropped: &self.dropped,
            changed: &self.dropped_changed,
        };
        futures::future::pending().await
    }
}

struct BlockingProofreader {
    started: Mutex<bool>,
    dropped: Mutex<bool>,
    started_changed: Condvar,
    dropped_changed: Condvar,
}

impl BlockingProofreader {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            started: Mutex::new(false),
            dropped: Mutex::new(false),
            started_changed: Condvar::new(),
            dropped_changed: Condvar::new(),
        })
    }

    fn wait_until_started(&self) {
        wait_for_signal(
            &self.started,
            &self.started_changed,
            "proofreading did not start",
        );
    }

    fn wait_until_dropped(&self) {
        wait_for_signal(
            &self.dropped,
            &self.dropped_changed,
            "proofreading did not stop",
        );
    }
}

#[async_trait::async_trait]
impl Proofreader for BlockingProofreader {
    async fn proofread(
        &self,
        _request: &ProofreadingRequest,
        _cancellation: &CancellationToken,
    ) -> Result<ProofreaderResponse, ProofreaderError> {
        *self.started.lock().unwrap() = true;
        self.started_changed.notify_all();
        let _drop_signal = DropSignal {
            dropped: &self.dropped,
            changed: &self.dropped_changed,
        };
        futures::future::pending().await
    }
}

struct DropSignal<'a> {
    dropped: &'a Mutex<bool>,
    changed: &'a Condvar,
}

impl Drop for DropSignal<'_> {
    fn drop(&mut self) {
        *self.dropped.lock().unwrap() = true;
        self.changed.notify_all();
    }
}

fn wait_for_signal(signal: &Mutex<bool>, changed: &Condvar, failure: &str) {
    let signal = signal.lock().unwrap();
    let (signal, result) = changed
        .wait_timeout_while(signal, WAIT_TIMEOUT, |signal| !*signal)
        .unwrap();
    assert!(*signal && !result.timed_out(), "{failure}");
}

struct RecordingProofreader {
    responses: Mutex<VecDeque<Result<ProofreaderResponse, ProofreaderError>>>,
    requests: Mutex<Vec<ProofreadingRequest>>,
}

impl RecordingProofreader {
    fn corrected(corrected_text: &'static str, explanation: &'static str) -> Arc<Self> {
        Arc::new(Self {
            responses: Mutex::new(VecDeque::from([Ok(ProofreaderResponse::Corrected(
                ProofreadingCorrection::new(corrected_text, explanation),
            ))])),
            requests: Mutex::new(Vec::new()),
        })
    }

    fn no_issues() -> Arc<Self> {
        Arc::new(Self {
            responses: Mutex::new(VecDeque::from([Ok(ProofreaderResponse::NoIssues)])),
            requests: Mutex::new(Vec::new()),
        })
    }
}

#[async_trait::async_trait]
impl Proofreader for RecordingProofreader {
    async fn proofread(
        &self,
        request: &ProofreadingRequest,
        _cancellation: &CancellationToken,
    ) -> Result<ProofreaderResponse, ProofreaderError> {
        self.requests.lock().unwrap().push(request.clone());
        self.responses
            .lock()
            .unwrap()
            .pop_front()
            .expect("the workflow should have a queued proofreading response")
    }
}

#[derive(Default)]
struct MemoryTranslationSettingsStore {
    target_language: Mutex<Option<LanguageIdentifier>>,
}

impl TranslationSettingsStore for MemoryTranslationSettingsStore {
    fn load_target_language(
        &self,
    ) -> Result<Option<LanguageIdentifier>, TranslationSettingsStoreError> {
        Ok(self.target_language.lock().unwrap().clone())
    }

    fn save_target_language(
        &self,
        target_language: &LanguageIdentifier,
    ) -> Result<(), TranslationSettingsStoreError> {
        *self.target_language.lock().unwrap() = Some(target_language.clone());
        Ok(())
    }
}

fn language(identifier: &str) -> LanguageIdentifier {
    LanguageIdentifier::new(identifier).unwrap()
}

fn preferences(store: Arc<MemoryTranslationSettingsStore>) -> Arc<TranslationPreferences> {
    let preferences = Arc::new(TranslationPreferences::load(store).unwrap());
    preferences
        .set_supported_targets([language("en"), language("fr")])
        .unwrap();
    preferences
}

fn workflow(
    selections: impl IntoIterator<Item = &'static str>,
    translator: Arc<dyn NativeTranslator>,
    proofreader: Arc<dyn Proofreader>,
    preferences: Arc<TranslationPreferences>,
) -> (
    Arc<ShortcutCoordinator>,
    WorkflowShortcutRegistry,
    Arc<PopupRecorder>,
) {
    let popup = Arc::new(PopupRecorder::default());
    let processor = Arc::new(ApplicationProcessor::new(
        translator,
        preferences,
        proofreader,
    ));
    let coordinator = Arc::new(ShortcutCoordinator::new(
        QueueCapture::new(selections),
        processor,
        popup.clone(),
    ));
    let mut registry = WorkflowShortcutRegistry::default();
    coordinator
        .register_shortcuts(&mut registry, &ShortcutConfiguration::default())
        .unwrap();

    (coordinator, registry, popup)
}

#[test]
fn translation_runs_from_shortcut_through_the_popup_view_model() {
    let translator = RecordingNativeTranslator::new(["Hello world"]);
    let (coordinator, registry, popup) = workflow(
        ["Hallo Welt"],
        translator.clone(),
        RecordingProofreader::no_issues(),
        preferences(Arc::new(MemoryTranslationSettingsStore::default())),
    );

    registry.trigger(TextAction::Translate);

    let updates = popup.wait_for_count(2);
    assert_eq!(
        updates,
        vec![
            (
                1,
                PresentationViewModel::Loading {
                    action: PresentationAction::Translate,
                },
            ),
            (
                1,
                PresentationViewModel::Translation {
                    original_text: "Hallo Welt".to_owned(),
                    language_pair: LanguagePairViewModel {
                        source: "de".to_owned(),
                        target: "en".to_owned(),
                    },
                    translated_text: "Hello world".to_owned(),
                },
            ),
        ]
    );
    assert_eq!(
        translator.requests.lock().unwrap().as_slice(),
        &[NativeTranslationRequest {
            text: "Hallo Welt".to_owned(),
            target_language_identifier: "en".to_owned(),
        }]
    );
    coordinator.shutdown();
}

#[test]
fn proofreading_runs_from_shortcut_through_the_popup_view_model() {
    let proofreader =
        RecordingProofreader::corrected("This is correct.", "Added the missing verb.");
    let (coordinator, registry, popup) = workflow(
        ["This correct."],
        RecordingNativeTranslator::new([]),
        proofreader.clone(),
        preferences(Arc::new(MemoryTranslationSettingsStore::default())),
    );

    registry.trigger(TextAction::Proofread);

    let updates = popup.wait_for_count(2);
    assert_eq!(
        updates,
        vec![
            (
                1,
                PresentationViewModel::Loading {
                    action: PresentationAction::Proofread,
                },
            ),
            (
                1,
                PresentationViewModel::Proofreading {
                    original_text: "This correct.".to_owned(),
                    corrected_text: "This is correct.".to_owned(),
                    explanation: "Added the missing verb.".to_owned(),
                },
            ),
        ]
    );
    let requests = proofreader.requests.lock().unwrap();
    assert_eq!(requests[0].text(), "This correct.");
    assert!(requests[0].policy().preserves_language());
    assert!(requests[0].policy().preserves_tone());
    assert!(requests[0].policy().preserves_whitespace());
    assert!(requests[0].policy().preserves_formatting());
    coordinator.shutdown();
}

#[test]
fn cancelling_an_active_translation_returns_the_popup_to_idle() {
    let translator = BlockingNativeTranslator::new();
    let (coordinator, registry, popup) = workflow(
        ["Hallo"],
        translator.clone(),
        RecordingProofreader::no_issues(),
        preferences(Arc::new(MemoryTranslationSettingsStore::default())),
    );

    registry.trigger(TextAction::Translate);
    translator.wait_until_started();
    assert!(coordinator.cancel_active());
    translator.wait_until_dropped();

    let updates = popup.wait_for_count(2);
    assert_eq!(updates.last().unwrap().1, PresentationViewModel::Idle);
    assert!(
        !updates
            .iter()
            .any(|(_, state)| matches!(state, PresentationViewModel::Translation { .. }))
    );
}

#[test]
fn cancelling_proofreading_drops_the_pending_provider_request() {
    let proofreader = BlockingProofreader::new();
    let (coordinator, registry, popup) = workflow(
        ["This needs work."],
        RecordingNativeTranslator::new([]),
        proofreader.clone(),
        preferences(Arc::new(MemoryTranslationSettingsStore::default())),
    );

    registry.trigger(TextAction::Proofread);
    proofreader.wait_until_started();
    assert!(coordinator.cancel_active());
    proofreader.wait_until_dropped();

    let updates = popup.wait_for_count(2);
    assert_eq!(updates.last().unwrap().1, PresentationViewModel::Idle);
}

#[test]
fn shutdown_cancels_work_without_publishing_a_new_popup_state() {
    let translator = BlockingNativeTranslator::new();
    let (coordinator, registry, popup) = workflow(
        ["Hallo"],
        translator.clone(),
        RecordingProofreader::no_issues(),
        preferences(Arc::new(MemoryTranslationSettingsStore::default())),
    );

    registry.trigger(TextAction::Translate);
    translator.wait_until_started();
    coordinator.shutdown();
    translator.wait_until_dropped();

    assert_eq!(
        popup.wait_for_count(1),
        vec![(
            1,
            PresentationViewModel::Loading {
                action: PresentationAction::Translate,
            },
        )]
    );
}

#[test]
fn a_different_overlapping_shortcut_replaces_the_active_workflow() {
    let translator = BlockingNativeTranslator::new();
    let proofreader = RecordingProofreader::corrected("This is right.", "Fixed grammar.");
    let (coordinator, registry, popup) = workflow(
        ["Hallo", "This right."],
        translator.clone(),
        proofreader,
        preferences(Arc::new(MemoryTranslationSettingsStore::default())),
    );

    registry.trigger(TextAction::Translate);
    translator.wait_until_started();
    registry.trigger(TextAction::Proofread);

    let updates = popup.wait_until(|updates| {
        updates.iter().any(|(_, state)| {
            matches!(
                state,
                PresentationViewModel::Proofreading {
                    corrected_text,
                    ..
                } if corrected_text == "This is right."
            )
        })
    });
    assert_eq!(
        updates.last().unwrap().1,
        PresentationViewModel::Proofreading {
            original_text: "This right.".to_owned(),
            corrected_text: "This is right.".to_owned(),
            explanation: "Fixed grammar.".to_owned(),
        }
    );
    assert!(
        !updates
            .iter()
            .any(|(_, state)| matches!(state, PresentationViewModel::Translation { .. }))
    );
    coordinator.shutdown();
}

#[test]
fn shortcuts_still_drive_the_workflow_after_sleep_and_wake_registration() {
    let translator = RecordingNativeTranslator::new(["Hello"]);
    let (coordinator, mut registry, popup) = workflow(
        ["Hallo"],
        translator,
        RecordingProofreader::no_issues(),
        preferences(Arc::new(MemoryTranslationSettingsStore::default())),
    );

    registry.unregister_all().unwrap();
    coordinator
        .register_shortcuts(&mut registry, &ShortcutConfiguration::default())
        .unwrap();
    registry.trigger(TextAction::Translate);

    let updates = popup.wait_for_count(2);
    assert!(matches!(
        updates.last().unwrap().1,
        PresentationViewModel::Translation { .. }
    ));
    assert_eq!(registry.registrations, 2);
    assert_eq!(registry.unregistrations, 1);
    coordinator.shutdown();
}

#[test]
fn target_language_changes_apply_to_the_next_shortcut_without_restarting() {
    let translator = RecordingNativeTranslator::new(["Hello", "Bonjour"]);
    let preferences = preferences(Arc::new(MemoryTranslationSettingsStore::default()));
    let (coordinator, registry, popup) = workflow(
        ["Hallo", "Hallo"],
        translator.clone(),
        RecordingProofreader::no_issues(),
        preferences.clone(),
    );

    registry.trigger(TextAction::Translate);
    popup.wait_for_count(2);
    preferences.set_target_language(language("fr")).unwrap();
    registry.trigger(TextAction::Translate);

    let updates = popup.wait_for_count(4);
    assert_eq!(
        translator
            .requests
            .lock()
            .unwrap()
            .iter()
            .map(|request| request.target_language_identifier.as_str())
            .collect::<Vec<_>>(),
        vec!["en", "fr"]
    );
    assert!(matches!(
        updates.last().unwrap().1,
        PresentationViewModel::Translation {
            ref language_pair,
            ..
        } if language_pair.target == "fr"
    ));
    coordinator.shutdown();
}

#[test]
fn a_relaunched_workflow_loads_the_persisted_target_language() {
    let store = Arc::new(MemoryTranslationSettingsStore::default());
    let first_launch_preferences = preferences(store.clone());
    first_launch_preferences
        .set_target_language(language("fr"))
        .unwrap();
    drop(first_launch_preferences);

    let translator = RecordingNativeTranslator::new(["Bonjour"]);
    let (coordinator, registry, popup) = workflow(
        ["Hallo"],
        translator.clone(),
        RecordingProofreader::no_issues(),
        preferences(store),
    );

    registry.trigger(TextAction::Translate);

    let updates = popup.wait_for_count(2);
    assert_eq!(
        translator.requests.lock().unwrap()[0].target_language_identifier,
        "fr"
    );
    assert!(matches!(
        updates.last().unwrap().1,
        PresentationViewModel::Translation {
            ref language_pair,
            ..
        } if language_pair.target == "fr"
    ));
    coordinator.shutdown();
}
