extern crate std;
use super::*;
use std::{cell::RefCell, collections::VecDeque, rc::Rc, vec, vec::Vec};

const OBJECTS: [ExpectedObject; 3] = [
    ExpectedObject {
        id: 0x7fff_1001,
        kind: 0x0b,
    },
    ExpectedObject {
        id: 0x7fff_1002,
        kind: 0x0c,
    },
    ExpectedObject {
        id: 0x7fff_1003,
        kind: 0x11,
    },
];
#[derive(Default)]
struct Db {
    pages: VecDeque<(u16, Vec<u8>)>,
    types: VecDeque<(u32, Vec<u8>)>,
    calls: usize,
    closes: usize,
    fail_at: Option<usize>,
    overreport: bool,
}
type Shared = Rc<RefCell<Db>>;
struct Io(Shared);
impl crate::private::Sealed for Io {}
impl Exchange for Io {
    fn exchange(&mut self, c: &Command<'_>, out: &mut [u8]) -> Result<usize, Error> {
        let mut db = self.0.borrow_mut();
        db.calls += 1;
        assert_eq!((c.class, c.instruction, c.p1), (0x80, 2, 0));
        if db.fail_at == Some(db.calls) {
            return Err(Error::Channel);
        }
        let response = match c.p2 {
            0x25 => {
                let (offset, response) = db.pages.pop_front().unwrap();
                let [hi, lo] = offset.to_be_bytes();
                assert_eq!(c.data, [0x41, 2, hi, lo, 0x42, 1, 0xff]);
                assert_eq!(c.le, Some(Le::Extended(512)));
                response
            }
            0x26 => {
                let (id, response) = db.types.pop_front().unwrap();
                assert_eq!(c.data[..2], [0x41, 4]);
                assert_eq!(c.data[2..], id.to_be_bytes());
                assert_eq!(c.le, None);
                response
            }
            _ => panic!("not an inventory command"),
        };
        let n = response.len().min(out.len());
        out[..n].copy_from_slice(&response[..n]);
        Ok(if db.overreport {
            out.len() + 1
        } else {
            response.len()
        })
    }
    fn close(&mut self) {
        self.0.borrow_mut().closes += 1;
    }
}
#[derive(Default)]
struct Check {
    calls: usize,
    fail_at: Option<usize>,
}
impl Checkpoint for Check {
    fn check(&mut self) -> Result<(), Error> {
        self.calls += 1;
        if self.fail_at == Some(self.calls) {
            Err(Error::Authorization)
        } else {
            Ok(())
        }
    }
}
fn db(pages: &[(u16, Vec<u8>)], objects: &[ExpectedObject]) -> Shared {
    Rc::new(RefCell::new(Db {
        pages: pages.iter().cloned().collect(),
        types: objects
            .iter()
            .map(|o| (o.id, vec![0x41, 1, o.kind, 0x42, 1, 1]))
            .collect(),
        ..Db::default()
    }))
}
fn valid_pages() -> Vec<(u16, Vec<u8>)> {
    // Literal independent transcript: MORE=02, NO_MORE=01; the first page is
    // deliberately unordered. Offsets count object IDs, not bytes or pages.
    vec![
        (
            0,
            vec![
                0x41, 1, 2, 0x42, 8, 0x7f, 0xff, 0x10, 3, 0x7f, 0xff, 0x10, 1,
            ],
        ),
        (2, vec![0x41, 1, 1, 0x42, 4, 0x7f, 0xff, 0x10, 2]),
    ]
}
#[test]
fn exact_authenticated_inventory_checks_offsets_types_and_all_pages() {
    let db = db(&valid_pages(), &OBJECTS);
    let baseline = Baseline::from_qualified_profile(&OBJECTS, [0xa7; 32]).unwrap();
    let report = verify_baseline(&mut Io(db.clone()), &baseline, &mut Check::default()).unwrap();
    assert_eq!(report.object_count(), 3);
    assert_eq!(report.profile_digest(), &[0xa7; 32]);
    assert_ne!(report.inventory_digest(), &[0; 32]);
    let db = db.borrow();
    assert_eq!((db.calls, db.closes), (5, 0));
    assert!(db.pages.is_empty() && db.types.is_empty());
}
#[test]
fn unqualified_or_openkeys_object_cannot_be_a_factory_allowlist() {
    for objects in [
        vec![],
        vec![OBJECTS[0]; 65],
        vec![OBJECTS[0]; 2],
        vec![OBJECTS[1], OBJECTS[0]],
        vec![ExpectedObject {
            id: crate::native_pin::PIN_ID,
            kind: 0x0c,
        }],
        vec![ExpectedObject {
            id: 0x7ffe_ffff,
            kind: 0x0b,
        }],
        vec![ExpectedObject {
            id: 0x8000_0000,
            kind: 0x0b,
        }],
        vec![ExpectedObject {
            id: OBJECTS[0].id,
            kind: 0,
        }],
    ] {
        assert!(Baseline::from_qualified_profile(&objects, [0xa7; 32]).is_err());
    }
    assert!(Baseline::from_qualified_profile(&OBJECTS, [0; 32]).is_err());
}
#[test]
fn zero_more_marker_noncanonical_tlv_partial_and_trailing_data_never_end_audit() {
    for response in [
        vec![],
        vec![0x41, 1, 0, 0x42, 0], // SDK NA is not NO_MORE.
        vec![0x41, 1, 3, 0x42, 0],
        vec![0x41, 1, 2, 0x42, 0], // MORE without progress.
        vec![0x41, 1, 1, 0x42, 0x81, 4, 0x7f, 0xff, 0x10, 1],
        vec![0x41, 1, 1, 0x42, 0x82, 0, 4, 0x7f, 0xff, 0x10, 1],
        vec![0x41, 1, 1, 0x42, 0x80, 0, 0],
        vec![0x41, 1, 1, 0x42, 3, 0x7f, 0xff, 0x10],
        vec![0x41, 1, 1, 0x42, 4, 0x7f, 0xff, 0x10, 1, 0],
        vec![0x41, 2, 1, 0, 0x42, 0],
        vec![0x41, 1, 1, 0x41, 0],
        vec![0x41, 1, 1, 0x42, 0x82, 2],
    ] {
        let db = db(&[(0, response)], &[]);
        let baseline = Baseline::from_qualified_profile(&OBJECTS, [0xa7; 32]).unwrap();
        assert!(verify_baseline(&mut Io(db.clone()), &baseline, &mut Check::default()).is_err());
        assert_eq!((db.borrow().calls, db.borrow().closes), (1, 1));
    }
}
#[test]
fn missing_unknown_duplicate_or_repeated_pages_cannot_hide_old_secrets() {
    for pages in [
        vec![(0, vec![0x41, 1, 1, 0x42, 0])],
        vec![(0, vec![0x41, 1, 1, 0x42, 4, 0x7f, 0xff, 0x10, 1])],
        vec![(0, vec![0x41, 1, 1, 0x42, 4, 0x4f, 0x4b, 8, 1])],
        vec![(
            0,
            vec![
                0x41, 1, 1, 0x42, 8, 0x7f, 0xff, 0x10, 1, 0x7f, 0xff, 0x10, 1,
            ],
        )],
        vec![
            (0, vec![0x41, 1, 2, 0x42, 4, 0x7f, 0xff, 0x10, 1]),
            (1, vec![0x41, 1, 1, 0x42, 4, 0x7f, 0xff, 0x10, 1]),
        ],
    ] {
        let db = db(&pages, &[]);
        let baseline = Baseline::from_qualified_profile(&OBJECTS, [0xa7; 32]).unwrap();
        assert!(verify_baseline(&mut Io(db.clone()), &baseline, &mut Check::default()).is_err());
        assert_eq!(db.borrow().closes, 1);
    }
}
#[test]
fn canonical_ber_81_82_and_full_64_bit_seen_mask_are_supported() {
    for (count, prefix) in [
        (32, vec![0x41, 1, 1, 0x42, 0x81, 0x80]),
        (63, vec![0x41, 1, 1, 0x42, 0x81, 0xfc]),
        (64, vec![0x41, 1, 1, 0x42, 0x82, 1, 0]),
    ] {
        let objects: Vec<_> = (0..count)
            .map(|i| ExpectedObject {
                id: 0x7fff_1000 + i,
                kind: 0x0b,
            })
            .collect();
        let mut response = prefix;
        for object in objects.iter().rev() {
            response.extend(object.id.to_be_bytes());
        }
        let db = db(&[(0, response)], &objects);
        let baseline = Baseline::from_qualified_profile(&objects, [0xa7; 32]).unwrap();
        let report =
            verify_baseline(&mut Io(db.clone()), &baseline, &mut Check::default()).unwrap();
        assert_eq!(report.object_count(), count as usize);
        assert_eq!(db.borrow().calls, count as usize + 1);
    }
}
#[test]
fn every_root_checkpoint_and_uncertain_io_closes_without_retry_or_report() {
    for fail_at in 1..=11 {
        let db = db(&valid_pages(), &OBJECTS);
        let baseline = Baseline::from_qualified_profile(&OBJECTS, [0xa7; 32]).unwrap();
        let mut check = Check {
            fail_at: Some(fail_at),
            ..Check::default()
        };
        assert!(verify_baseline(&mut Io(db.clone()), &baseline, &mut check).is_err());
        assert_eq!(db.borrow().closes, 1);
        assert_eq!(check.calls, fail_at);
    }
    for fail_at in 1..=5 {
        let db = db(&valid_pages(), &OBJECTS);
        db.borrow_mut().fail_at = Some(fail_at);
        let baseline = Baseline::from_qualified_profile(&OBJECTS, [0xa7; 32]).unwrap();
        assert!(verify_baseline(&mut Io(db.clone()), &baseline, &mut Check::default()).is_err());
        assert_eq!((db.borrow().calls, db.borrow().closes), (fail_at, 1));
    }
}
#[test]
fn wrong_transience_type_trailing_attributes_or_length_overreport_is_rejected() {
    for response in [
        vec![0x41, 1, 0x0b, 0x42, 1, 2],
        vec![0x41, 1, 0x0c, 0x42, 1, 1],
        vec![0x41, 1, 0x0b, 0x42, 1, 1, 0],
    ] {
        let db = db(&valid_pages(), &OBJECTS);
        db.borrow_mut().types.front_mut().unwrap().1 = response;
        let baseline = Baseline::from_qualified_profile(&OBJECTS, [0xa7; 32]).unwrap();
        assert!(verify_baseline(&mut Io(db.clone()), &baseline, &mut Check::default()).is_err());
        assert_eq!(db.borrow().closes, 1);
    }
    let db = db(&valid_pages(), &OBJECTS);
    db.borrow_mut().overreport = true;
    let baseline = Baseline::from_qualified_profile(&OBJECTS, [0xa7; 32]).unwrap();
    assert!(verify_baseline(&mut Io(db.clone()), &baseline, &mut Check::default()).is_err());
    assert_eq!((db.borrow().calls, db.borrow().closes), (1, 1));
}
