//! Public AT-SPI fixtures for the zbus-6 migration. No owner or device data.
#![cfg(all(feature = "wire-tests", target_os = "linux"))]
use atspi_common::{ObjectRef, State, events::object::StateChangedEvent};
use zbus::{
    Message, ObjectPath, Type,
    names::UniqueName,
    wire::{LE, serialized::Context, to_bytes},
};

#[test]
fn accessibility_object_reference_has_canonical_dbus_wire_and_roundtrips() {
    let object = ObjectRef::new(
        UniqueName::try_from(":1.42").unwrap(),
        ObjectPath::try_from("/openkeys").unwrap(),
    );
    assert_eq!(ObjectRef::SIGNATURE.to_string(), "(so)");
    let bytes = to_bytes(Context::new(LE, 0), &object).unwrap();
    // D-Bus struct: little-endian string length, NUL, alignment, object path.
    assert_eq!(bytes.bytes(), b"\x05\0\0\0:1.42\0\0\0\x09\0\0\0/openkeys\0");
    let (decoded, consumed): (ObjectRef<'_>, _) = bytes.deserialize().unwrap();
    assert_eq!(decoded, object);
    assert_eq!(consumed, bytes.len());
}

#[test]
fn accessibility_signal_preserves_type_and_rejects_unrelated_body() {
    let event = StateChangedEvent {
        item: ObjectRef::new_owned(
            UniqueName::try_from(":1.42").unwrap(),
            ObjectPath::try_from("/openkeys").unwrap(),
        ),
        state: State::Focused,
        enabled: true,
    };
    let message: Message = event.clone().try_into().unwrap();
    assert_eq!(StateChangedEvent::try_from(&message).unwrap(), event);
    let wrong = Message::signal("/openkeys", "org.a11y.atspi.Event.Object", "StateChanged")
        .sender(UniqueName::try_from(":1.42").unwrap())
        .build(&42u32)
        .unwrap();
    assert!(StateChangedEvent::try_from(&wrong).is_err());
}
