use objc2::{rc::Retained, runtime::ProtocolObject};
use objc2_app_kit::{NSPasteboard, NSPasteboardItem, NSPasteboardWriting};
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
    WriteFailed,
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
        let Some(items) = pasteboard.pasteboardItems() else {
            return Ok(Vec::new());
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

    fn replace_items(
        &self,
        items: &[PasteboardItemSnapshot],
    ) -> Result<i64, PasteboardSnapshotError> {
        let pasteboard = NSPasteboard::generalPasteboard();
        let pasteboard_items = items
            .iter()
            .map(|snapshot| {
                let item = NSPasteboardItem::new();
                for representation in &snapshot.representations {
                    let type_identifier = NSString::from_str(&representation.type_identifier);
                    let data = NSData::with_bytes(&representation.data);
                    if !item.setData_forType(&data, &type_identifier) {
                        return Err(PasteboardSnapshotError::WriteFailed);
                    }
                }
                Ok(ProtocolObject::from_retained(item))
            })
            .collect::<Result<Vec<Retained<ProtocolObject<dyn NSPasteboardWriting>>>, _>>()?;
        let pasteboard_items = NSArray::from_retained_slice(&pasteboard_items);

        pasteboard.clearContents();

        if items.is_empty() {
            return Ok(pasteboard.changeCount() as i64);
        }

        if !pasteboard.writeObjects(&pasteboard_items) {
            return Err(PasteboardSnapshotError::WriteFailed);
        }

        Ok(pasteboard.changeCount() as i64)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};

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
