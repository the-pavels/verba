use objc2::{rc::Retained, runtime::ProtocolObject};
use objc2_app_kit::{NSPasteboard, NSPasteboardItem, NSPasteboardTypeString, NSPasteboardWriting};
use objc2_foundation::{NSArray, NSData, NSString};

#[derive(Clone, Debug, Eq, PartialEq)]
struct PasteboardRepresentation {
    type_identifier: String,
    data: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PasteboardItemSnapshot {
    representations: Vec<PasteboardRepresentation>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PasteboardSnapshot {
    items: Vec<PasteboardItemSnapshot>,
    change_count: i64,
}

impl PasteboardSnapshot {
    #[must_use]
    pub fn change_count(&self) -> i64 {
        self.change_count
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PasteboardRestoreOutcome {
    Restored { change_count: i64 },
    SkippedDueToConflict,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PasteboardSnapshotError {
    ChangedDuringSnapshot,
    ReadFailed,
    WritePreparationFailed,
    WriteFailed,
    ChangedDuringWrite,
    WriteVerificationFailed,
}

pub struct MacOsPasteboard {
    inner: SnapshotService<AppKitPasteboard>,
}

impl MacOsPasteboard {
    #[must_use]
    pub fn general() -> Self {
        Self {
            inner: SnapshotService::new(AppKitPasteboard),
        }
    }

    pub fn snapshot(&self) -> Result<PasteboardSnapshot, PasteboardSnapshotError> {
        self.inner.snapshot()
    }

    #[must_use]
    pub fn change_count(&self) -> i64 {
        self.inner.change_count()
    }

    pub fn restore(
        &self,
        snapshot: &PasteboardSnapshot,
        expected_change_count: i64,
    ) -> Result<PasteboardRestoreOutcome, PasteboardSnapshotError> {
        self.inner.restore(snapshot, expected_change_count)
    }

    #[must_use]
    pub fn plain_text(&self) -> Option<String> {
        let string_type = unsafe { NSPasteboardTypeString };
        NSPasteboard::generalPasteboard()
            .stringForType(string_type)
            .map(|text| text.to_string())
    }
}

impl Default for MacOsPasteboard {
    fn default() -> Self {
        Self::general()
    }
}

struct SnapshotService<B> {
    backend: B,
}

impl<B: PasteboardBackend> SnapshotService<B> {
    fn new(backend: B) -> Self {
        Self { backend }
    }

    fn snapshot(&self) -> Result<PasteboardSnapshot, PasteboardSnapshotError> {
        let change_count = self.backend.change_count();
        let items = self.backend.read_items()?;

        if self.backend.change_count() != change_count {
            return Err(PasteboardSnapshotError::ChangedDuringSnapshot);
        }

        Ok(PasteboardSnapshot {
            items,
            change_count,
        })
    }

    fn change_count(&self) -> i64 {
        self.backend.change_count()
    }

    fn restore(
        &self,
        snapshot: &PasteboardSnapshot,
        expected_change_count: i64,
    ) -> Result<PasteboardRestoreOutcome, PasteboardSnapshotError> {
        if self.backend.change_count() != expected_change_count {
            return Ok(PasteboardRestoreOutcome::SkippedDueToConflict);
        }

        self.backend
            .replace_items(&snapshot.items)
            .map(|change_count| PasteboardRestoreOutcome::Restored { change_count })
    }
}

trait PasteboardBackend {
    fn change_count(&self) -> i64;

    fn read_items(&self) -> Result<Vec<PasteboardItemSnapshot>, PasteboardSnapshotError>;

    fn replace_items(
        &self,
        items: &[PasteboardItemSnapshot],
    ) -> Result<i64, PasteboardSnapshotError>;
}

struct AppKitPasteboard;

impl PasteboardBackend for AppKitPasteboard {
    fn change_count(&self) -> i64 {
        NSPasteboard::generalPasteboard().changeCount() as i64
    }

    fn read_items(&self) -> Result<Vec<PasteboardItemSnapshot>, PasteboardSnapshotError> {
        let pasteboard = NSPasteboard::generalPasteboard();
        read_appkit_items(&pasteboard)
    }

    fn replace_items(
        &self,
        items: &[PasteboardItemSnapshot],
    ) -> Result<i64, PasteboardSnapshotError> {
        let pasteboard = NSPasteboard::generalPasteboard();
        replace_items(
            &AppKitPasteboardWriter {
                pasteboard: &pasteboard,
            },
            items,
        )
    }
}

fn read_appkit_items(
    pasteboard: &NSPasteboard,
) -> Result<Vec<PasteboardItemSnapshot>, PasteboardSnapshotError> {
    let Some(items) = pasteboard.pasteboardItems() else {
        return Err(PasteboardSnapshotError::ReadFailed);
    };

    items
        .to_vec()
        .into_iter()
        .map(|item| {
            let representations = item
                .types()
                .to_vec()
                .into_iter()
                .map(|type_identifier| {
                    let data = item
                        .dataForType(&type_identifier)
                        .ok_or(PasteboardSnapshotError::ReadFailed)?;
                    Ok(PasteboardRepresentation {
                        type_identifier: type_identifier.to_string(),
                        data: data.to_vec(),
                    })
                })
                .collect::<Result<Vec<_>, PasteboardSnapshotError>>()?;

            Ok(PasteboardItemSnapshot { representations })
        })
        .collect()
}

fn read_appkit_item_types(
    pasteboard: &NSPasteboard,
) -> Result<Vec<Vec<String>>, PasteboardSnapshotError> {
    let items = pasteboard
        .pasteboardItems()
        .ok_or(PasteboardSnapshotError::ReadFailed)?;
    Ok(items
        .to_vec()
        .into_iter()
        .map(|item| {
            item.types()
                .to_vec()
                .into_iter()
                .map(|type_identifier| type_identifier.to_string())
                .collect()
        })
        .collect())
}

fn item_types(items: &[PasteboardItemSnapshot]) -> Vec<Vec<String>> {
    items
        .iter()
        .map(|item| {
            item.representations
                .iter()
                .map(|representation| representation.type_identifier.clone())
                .collect()
        })
        .collect()
}

fn contains_expected_item_types(written: &[Vec<String>], expected: &[Vec<String>]) -> bool {
    written.len() == expected.len()
        && written.iter().zip(expected).all(|(written, expected)| {
            expected
                .iter()
                .all(|type_identifier| written.contains(type_identifier))
        })
}

trait PasteboardWriteBackend {
    type PreparedItems;

    fn prepare_items(
        &self,
        items: &[PasteboardItemSnapshot],
    ) -> Result<Self::PreparedItems, PasteboardSnapshotError>;
    fn clear_contents(&self) -> i64;
    fn change_count(&self) -> i64;
    fn write_items(&self, items: &Self::PreparedItems) -> bool;
    fn read_item_types(&self) -> Result<Vec<Vec<String>>, PasteboardSnapshotError>;
}

fn replace_items<W: PasteboardWriteBackend>(
    backend: &W,
    items: &[PasteboardItemSnapshot],
) -> Result<i64, PasteboardSnapshotError> {
    let prepared = backend.prepare_items(items)?;
    let replacement_change_count = backend.clear_contents();

    if !items.is_empty() && !backend.write_items(&prepared) {
        if backend.change_count() != replacement_change_count {
            return Err(PasteboardSnapshotError::ChangedDuringWrite);
        }
        if !backend
            .read_item_types()
            .map_err(|_| PasteboardSnapshotError::WriteVerificationFailed)?
            .is_empty()
        {
            return Err(PasteboardSnapshotError::WriteVerificationFailed);
        }
        if backend.change_count() != replacement_change_count {
            return Err(PasteboardSnapshotError::ChangedDuringWrite);
        }
        if !backend.write_items(&prepared) {
            return Err(if backend.change_count() == replacement_change_count {
                PasteboardSnapshotError::WriteFailed
            } else {
                PasteboardSnapshotError::ChangedDuringWrite
            });
        }
    }

    if backend.change_count() != replacement_change_count {
        return Err(PasteboardSnapshotError::ChangedDuringWrite);
    }
    let written_types = backend
        .read_item_types()
        .map_err(|_| PasteboardSnapshotError::WriteVerificationFailed)?;
    if backend.change_count() != replacement_change_count {
        return Err(PasteboardSnapshotError::ChangedDuringWrite);
    }
    if !contains_expected_item_types(&written_types, &item_types(items)) {
        return Err(PasteboardSnapshotError::WriteVerificationFailed);
    }

    Ok(replacement_change_count)
}

struct AppKitPasteboardWriter<'a> {
    pasteboard: &'a NSPasteboard,
}

impl PasteboardWriteBackend for AppKitPasteboardWriter<'_> {
    type PreparedItems = Retained<NSArray<ProtocolObject<dyn NSPasteboardWriting>>>;

    fn prepare_items(
        &self,
        items: &[PasteboardItemSnapshot],
    ) -> Result<Self::PreparedItems, PasteboardSnapshotError> {
        let pasteboard_items = items
            .iter()
            .map(|snapshot| {
                let item = NSPasteboardItem::new();
                for representation in &snapshot.representations {
                    let type_identifier = NSString::from_str(&representation.type_identifier);
                    let data = NSData::with_bytes(&representation.data);
                    if !item.setData_forType(&data, &type_identifier) {
                        return Err(PasteboardSnapshotError::WritePreparationFailed);
                    }
                }
                Ok(ProtocolObject::from_retained(item))
            })
            .collect::<Result<Vec<Retained<ProtocolObject<dyn NSPasteboardWriting>>>, _>>()?;
        Ok(NSArray::from_retained_slice(&pasteboard_items))
    }

    fn clear_contents(&self) -> i64 {
        self.pasteboard.clearContents() as i64
    }

    fn change_count(&self) -> i64 {
        self.pasteboard.changeCount() as i64
    }

    fn write_items(&self, items: &Self::PreparedItems) -> bool {
        self.pasteboard.writeObjects(items)
    }

    fn read_item_types(&self) -> Result<Vec<Vec<String>>, PasteboardSnapshotError> {
        read_appkit_item_types(self.pasteboard)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        cell::{Cell, RefCell},
        collections::VecDeque,
    };

    use super::*;

    const TEXT: &str = "public.utf8-plain-text";
    const RTF: &str = "public.rtf";
    const HTML: &str = "public.html";
    const PNG: &str = "public.png";
    const FILE_URL: &str = "public.file-url";

    #[test]
    fn restores_text() {
        assert_round_trip(vec![item(&[(TEXT, b"hello")])]);
    }

    #[test]
    fn restores_all_rich_text_representations() {
        assert_round_trip(vec![item(&[
            (TEXT, b"formatted"),
            (RTF, b"{\\rtf1 formatted}"),
            (HTML, b"<b>formatted</b>"),
        ])]);
    }

    #[test]
    fn restores_image_data() {
        assert_round_trip(vec![item(&[(PNG, &[0x89, b'P', b'N', b'G'])])]);
    }

    #[test]
    fn restores_multiple_file_items() {
        assert_round_trip(vec![
            item(&[(FILE_URL, b"file:///tmp/one.txt")]),
            item(&[(FILE_URL, b"file:///tmp/two.pdf")]),
        ]);
    }

    #[test]
    fn restores_an_empty_clipboard() {
        assert_round_trip(Vec::new());
    }

    #[test]
    fn skips_restoration_after_an_external_change() {
        let original = vec![item(&[(TEXT, b"original")])];
        let backend = FakePasteboard::new(original);
        let service = SnapshotService::new(backend);
        let snapshot = service.snapshot().unwrap();

        let copied_count = service.backend.replace(vec![item(&[(TEXT, b"copied")])]);
        service.backend.replace(vec![item(&[(TEXT, b"external")])]);

        assert_eq!(
            service.restore(&snapshot, copied_count).unwrap(),
            PasteboardRestoreOutcome::SkippedDueToConflict
        );
        assert_eq!(service.backend.items(), vec![item(&[(TEXT, b"external")])]);
    }

    #[test]
    fn rejects_a_snapshot_that_changes_while_being_read() {
        let backend = FakePasteboard::new(vec![item(&[(TEXT, b"original")])]);
        backend.change_during_next_read.set(true);
        let service = SnapshotService::new(backend);

        assert_eq!(
            service.snapshot(),
            Err(PasteboardSnapshotError::ChangedDuringSnapshot)
        );
    }

    #[test]
    fn preparation_failure_leaves_the_current_clipboard_untouched() {
        let original = vec![item(&[(TEXT, b"original")])];
        let backend = FakeWriteBackend::new(original.clone(), [WriteOutcome::Success]);
        backend.preparation_fails.set(true);

        assert_eq!(
            replace_items(&backend, &[item(&[(TEXT, b"replacement")])]),
            Err(PasteboardSnapshotError::WritePreparationFailed)
        );
        assert_eq!(backend.items(), original);
        assert_eq!(backend.change_count(), 1);
        assert_eq!(backend.write_calls.get(), 0);
    }

    #[test]
    fn retries_once_when_a_failed_write_left_the_owned_clipboard_empty() {
        let replacement = vec![item(&[(TEXT, b"replacement")])];
        let backend = FakeWriteBackend::new(
            vec![item(&[(TEXT, b"original")])],
            [WriteOutcome::FailEmpty, WriteOutcome::Success],
        );

        assert_eq!(replace_items(&backend, &replacement), Ok(2));
        assert_eq!(backend.items(), replacement);
        assert_eq!(backend.write_calls.get(), 2);
    }

    #[test]
    fn reports_a_failed_retry_after_clear_with_the_empty_state_visible() {
        let backend = FakeWriteBackend::new(
            vec![item(&[(TEXT, b"original")])],
            [WriteOutcome::FailEmpty, WriteOutcome::FailEmpty],
        );

        assert_eq!(
            replace_items(&backend, &[item(&[(TEXT, b"replacement")])]),
            Err(PasteboardSnapshotError::WriteFailed)
        );
        assert!(backend.items().is_empty());
        assert_eq!(backend.write_calls.get(), 2);
    }

    #[test]
    fn detects_a_partial_write_without_appending_a_retry() {
        let partial = vec![item(&[(TEXT, b"partial")])];
        let backend = FakeWriteBackend::new(
            vec![item(&[(TEXT, b"original")])],
            [WriteOutcome::FailWith(partial.clone())],
        );

        assert_eq!(
            replace_items(
                &backend,
                &[item(&[(TEXT, b"first")]), item(&[(TEXT, b"second")]),]
            ),
            Err(PasteboardSnapshotError::WriteVerificationFailed)
        );
        assert_eq!(backend.items(), partial);
        assert_eq!(backend.write_calls.get(), 1);
    }

    #[test]
    fn never_retries_over_a_newer_external_clipboard_change() {
        let external = vec![item(&[(TEXT, b"external")])];
        let backend = FakeWriteBackend::new(
            vec![item(&[(TEXT, b"original")])],
            [WriteOutcome::ExternalChange(external.clone())],
        );

        assert_eq!(
            replace_items(&backend, &[item(&[(TEXT, b"replacement")])]),
            Err(PasteboardSnapshotError::ChangedDuringWrite)
        );
        assert_eq!(backend.items(), external);
        assert_eq!(backend.write_calls.get(), 1);
    }

    #[test]
    fn detects_an_external_change_before_post_write_verification() {
        let external = vec![item(&[(TEXT, b"external")])];
        let backend = FakeWriteBackend::new(
            vec![item(&[(TEXT, b"original")])],
            [WriteOutcome::SucceedThenExternal(external.clone())],
        );

        assert_eq!(
            replace_items(&backend, &[item(&[(TEXT, b"replacement")])]),
            Err(PasteboardSnapshotError::ChangedDuringWrite)
        );
        assert_eq!(backend.items(), external);
        assert_eq!(backend.write_calls.get(), 1);
    }

    #[test]
    fn does_not_retry_if_ownership_changes_while_the_empty_state_is_verified() {
        let external = vec![item(&[(TEXT, b"external")])];
        let backend = FakeWriteBackend::new(
            vec![item(&[(TEXT, b"original")])],
            [WriteOutcome::FailEmpty, WriteOutcome::Success],
        );
        backend
            .external_change_after_next_type_read
            .replace(Some(external.clone()));

        assert_eq!(
            replace_items(&backend, &[item(&[(TEXT, b"replacement")])]),
            Err(PasteboardSnapshotError::ChangedDuringWrite)
        );
        assert_eq!(backend.items(), external);
        assert_eq!(backend.write_calls.get(), 1);
    }

    #[test]
    fn detects_ownership_change_during_post_write_verification() {
        let external = vec![item(&[(TEXT, b"external")])];
        let backend =
            FakeWriteBackend::new(vec![item(&[(TEXT, b"original")])], [WriteOutcome::Success]);
        backend
            .external_change_after_next_type_read
            .replace(Some(external.clone()));

        assert_eq!(
            replace_items(&backend, &[item(&[(TEXT, b"replacement")])]),
            Err(PasteboardSnapshotError::ChangedDuringWrite)
        );
        assert_eq!(backend.items(), external);
        assert_eq!(backend.write_calls.get(), 1);
    }

    #[test]
    fn restoring_an_empty_snapshot_clears_without_attempting_a_write() {
        let backend = FakeWriteBackend::new(vec![item(&[(TEXT, b"copied")])], std::iter::empty());

        assert_eq!(replace_items(&backend, &[]), Ok(2));
        assert!(backend.items().is_empty());
        assert_eq!(backend.write_calls.get(), 0);
    }

    #[test]
    fn verifies_the_written_items_and_representations() {
        let unexpected = vec![item(&[(RTF, b"{\\rtf1 unexpected}")])];
        let backend = FakeWriteBackend::new(
            vec![item(&[(TEXT, b"original")])],
            [WriteOutcome::SucceedWith(unexpected.clone())],
        );

        assert_eq!(
            replace_items(&backend, &[item(&[(TEXT, b"replacement")])]),
            Err(PasteboardSnapshotError::WriteVerificationFailed)
        );
        assert_eq!(backend.items(), unexpected);
    }

    #[test]
    fn appkit_round_trips_large_multi_item_data_on_an_isolated_pasteboard() {
        let pasteboard = NSPasteboard::pasteboardWithUniqueName();
        let backend = AppKitPasteboardWriter {
            pasteboard: &pasteboard,
        };
        assert_eq!(read_appkit_items(&pasteboard).unwrap(), Vec::new());
        let large_data = vec![0xA5; 8 * 1024 * 1024];
        let original = vec![
            item(&[(TEXT, b"formatted"), (RTF, b"{\\rtf1 formatted}")]),
            item(&[(PNG, &large_data)]),
            item(&[(FILE_URL, b"file:///tmp/example.pdf")]),
        ];

        let change_count = replace_items(&backend, &original).unwrap();

        assert_eq!(backend.change_count(), change_count);
        assert!(contains_expected_item_types(
            &backend.read_item_types().unwrap(),
            &item_types(&original)
        ));
        let written_items = pasteboard.pasteboardItems().unwrap().to_vec();
        let png_type = NSString::from_str(PNG);
        assert_eq!(
            written_items[1].dataForType(&png_type).unwrap().len(),
            large_data.len()
        );
    }

    fn assert_round_trip(original: Vec<PasteboardItemSnapshot>) {
        let backend = FakePasteboard::new(original.clone());
        let service = SnapshotService::new(backend);
        let snapshot = service.snapshot().unwrap();
        let copied_count = service.backend.replace(vec![item(&[(TEXT, b"copied")])]);

        assert_eq!(
            service.restore(&snapshot, copied_count).unwrap(),
            PasteboardRestoreOutcome::Restored {
                change_count: copied_count + 1
            }
        );
        assert_eq!(service.backend.items(), original);
    }

    fn item(representations: &[(&str, &[u8])]) -> PasteboardItemSnapshot {
        PasteboardItemSnapshot {
            representations: representations
                .iter()
                .map(|(type_identifier, data)| PasteboardRepresentation {
                    type_identifier: (*type_identifier).to_owned(),
                    data: data.to_vec(),
                })
                .collect(),
        }
    }

    struct FakePasteboard {
        items: RefCell<Vec<PasteboardItemSnapshot>>,
        change_count: Cell<i64>,
        change_during_next_read: Cell<bool>,
    }

    enum WriteOutcome {
        Success,
        FailEmpty,
        FailWith(Vec<PasteboardItemSnapshot>),
        ExternalChange(Vec<PasteboardItemSnapshot>),
        SucceedThenExternal(Vec<PasteboardItemSnapshot>),
        SucceedWith(Vec<PasteboardItemSnapshot>),
    }

    struct FakeWriteBackend {
        items: RefCell<Vec<PasteboardItemSnapshot>>,
        change_count: Cell<i64>,
        preparation_fails: Cell<bool>,
        outcomes: RefCell<VecDeque<WriteOutcome>>,
        write_calls: Cell<usize>,
        external_change_after_next_type_read: RefCell<Option<Vec<PasteboardItemSnapshot>>>,
    }

    impl FakeWriteBackend {
        fn new(
            items: Vec<PasteboardItemSnapshot>,
            outcomes: impl IntoIterator<Item = WriteOutcome>,
        ) -> Self {
            Self {
                items: RefCell::new(items),
                change_count: Cell::new(1),
                preparation_fails: Cell::new(false),
                outcomes: RefCell::new(outcomes.into_iter().collect()),
                write_calls: Cell::new(0),
                external_change_after_next_type_read: RefCell::new(None),
            }
        }

        fn items(&self) -> Vec<PasteboardItemSnapshot> {
            self.items.borrow().clone()
        }
    }

    impl PasteboardWriteBackend for FakeWriteBackend {
        type PreparedItems = Vec<PasteboardItemSnapshot>;

        fn prepare_items(
            &self,
            items: &[PasteboardItemSnapshot],
        ) -> Result<Self::PreparedItems, PasteboardSnapshotError> {
            if self.preparation_fails.get() {
                Err(PasteboardSnapshotError::WritePreparationFailed)
            } else {
                Ok(items.to_vec())
            }
        }

        fn clear_contents(&self) -> i64 {
            self.items.borrow_mut().clear();
            let change_count = self.change_count.get() + 1;
            self.change_count.set(change_count);
            change_count
        }

        fn change_count(&self) -> i64 {
            self.change_count.get()
        }

        fn write_items(&self, items: &Self::PreparedItems) -> bool {
            self.write_calls.set(self.write_calls.get() + 1);
            match self.outcomes.borrow_mut().pop_front().unwrap() {
                WriteOutcome::Success => {
                    self.items.replace(items.clone());
                    true
                }
                WriteOutcome::FailEmpty => false,
                WriteOutcome::FailWith(partial) => {
                    self.items.replace(partial);
                    false
                }
                WriteOutcome::ExternalChange(external) => {
                    self.items.replace(external);
                    self.change_count.set(self.change_count.get() + 1);
                    false
                }
                WriteOutcome::SucceedThenExternal(external) => {
                    self.items.replace(external);
                    self.change_count.set(self.change_count.get() + 1);
                    true
                }
                WriteOutcome::SucceedWith(unexpected) => {
                    self.items.replace(unexpected);
                    true
                }
            }
        }

        fn read_item_types(&self) -> Result<Vec<Vec<String>>, PasteboardSnapshotError> {
            let types = item_types(&self.items());
            if let Some(external) = self
                .external_change_after_next_type_read
                .borrow_mut()
                .take()
            {
                self.items.replace(external);
                self.change_count.set(self.change_count.get() + 1);
            }
            Ok(types)
        }
    }

    impl FakePasteboard {
        fn new(items: Vec<PasteboardItemSnapshot>) -> Self {
            Self {
                items: RefCell::new(items),
                change_count: Cell::new(1),
                change_during_next_read: Cell::new(false),
            }
        }

        fn items(&self) -> Vec<PasteboardItemSnapshot> {
            self.items.borrow().clone()
        }

        fn replace(&self, items: Vec<PasteboardItemSnapshot>) -> i64 {
            *self.items.borrow_mut() = items;
            let change_count = self.change_count.get() + 1;
            self.change_count.set(change_count);
            change_count
        }
    }

    impl PasteboardBackend for FakePasteboard {
        fn change_count(&self) -> i64 {
            self.change_count.get()
        }

        fn read_items(&self) -> Result<Vec<PasteboardItemSnapshot>, PasteboardSnapshotError> {
            let items = self.items();
            if self.change_during_next_read.replace(false) {
                self.change_count.set(self.change_count.get() + 1);
            }
            Ok(items)
        }

        fn replace_items(
            &self,
            items: &[PasteboardItemSnapshot],
        ) -> Result<i64, PasteboardSnapshotError> {
            Ok(self.replace(items.to_vec()))
        }
    }
}
