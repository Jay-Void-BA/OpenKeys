//! English GPUI view, shared separately from native transport adapters.
use crate::{Action, Backend, OTP_POLICY, PIN_POLICY};
use gpui::{
    App, Bounds, Context, Div, FontWeight, SharedString, Window, WindowBounds, WindowOptions, div,
    prelude::*, px, rgb, size,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    Overview,
    Otp,
    OpenPgp,
    Piv,
    Ssh,
    Notes,
    Security,
}
impl Section {
    const ALL: [(Self, &'static str); 7] = [
        (Self::Overview, "Overview"),
        (Self::Otp, "TOTP / HOTP"),
        (Self::OpenPgp, "OpenPGP"),
        (Self::Piv, "PIV"),
        (Self::Ssh, "SSH"),
        (Self::Notes, "Notes & recovery"),
        (Self::Security, "Security"),
    ];
    fn title(self) -> &'static str {
        Self::ALL.iter().find(|(s, _)| *s == self).unwrap().1
    }
}
struct Manager {
    backend: Backend,
    section: Section,
    status: SharedString,
}
impl Manager {
    fn action(&mut self, action: Action, cx: &mut Context<Self>) {
        self.status = match self.backend.request(action) {
            Ok(()) => {
                "Request sent; complete the hardware PIN and exact request on your key.".into()
            }
            Err(_) => {
                "Secure device channel is not integrated. No action was sent; no data was read."
                    .into()
            }
        };
        cx.notify();
    }
    fn button(
        &self,
        id: &'static str,
        label: &'static str,
        action: Action,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<Div> {
        div()
            .id(id)
            .px_4()
            .py_2()
            .rounded_md()
            .bg(rgb(0x245d70))
            .text_color(rgb(0xffffff))
            .child(label)
            .on_click(cx.listener(move |this, _, _, cx| this.action(action, cx)))
    }
    fn panel(&self, cx: &mut Context<Self>) -> Div {
        let base = div()
            .flex()
            .flex_col()
            .gap_4()
            .p_6()
            .rounded_lg()
            .bg(rgb(0x18252f));
        match self.section{
            Section::Overview=>base.child("Local authorization, for every private operation")
                .child(PIN_POLICY).child("Connect the key via USB. Mobile NFC/USB integrations require platform verification.")
                .child(self.button("connect","Connect a key",Action::Connect,cx)),
            Section::Otp=>base.child(OTP_POLICY)
                .child("Import, list and delete require an authenticated hardware-PIN session and local approval. HOTP advances a persistent counter before display.")
                .child(self.button("import-otp","Import TOTP / HOTP",Action::ImportOtp,cx))
                .child(self.button("show-otp","Show code on key LCD",Action::ShowOtpOnKey,cx))
                .child(self.button("delete-otp","Delete OTP account",Action::DeleteOtp,cx)),
            Section::OpenPgp=>base.child("Signing, decryption and authentication")
                .child("OpenPGP user/admin PINs remain independent of the 6-digit hardware PIN.")
                .child(self.button("openpgp","Manage OpenPGP",Action::OpenPgp,cx)),
            Section::Piv=>base.child("Certificates and smart-card keys")
                .child("PIV PIN, PUK and management credentials do not replace the local hardware PIN.")
                .child(self.button("piv","Manage PIV",Action::Piv,cx)),
            Section::Ssh=>base.child("SSH through FIDO security keys, OpenPGP or PIV")
                .child("Each private-key operation requires local hardware authorization.")
                .child(self.button("ssh","Manage SSH public identities",Action::Ssh,cx)),
            Section::Notes=>base.child("Private notes and recovery codes")
                .child("No private titles, previews or records are shown while locked.")
                .child(self.button("notes","View private records",Action::ViewPrivateRecords,cx))
                .child(self.button("import-notes","Import notes / recovery codes",Action::ImportNotes,cx)),
            Section::Security=>base.child(PIN_POLICY)
                .child("Changing the hardware PIN erases ALL user data. The 11th incorrect completed guess also requires erasure.")
                .child("An interrupted erase must remain locked after reboot. Pairing and protocol PINs cannot bypass it.")
                .child(self.button("change-pin","Erase data and change hardware PIN",Action::ChangeHardwarePin,cx)),
        }
    }
}
impl Render for Manager {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let navigation = Section::ALL
            .into_iter()
            .map(|(section, label)| {
                let selected = self.section == section;
                div()
                    .id(label)
                    .px_4()
                    .py_3()
                    .rounded_md()
                    .bg(rgb(if selected { 0x25495c } else { 0x111e28 }))
                    .child(label)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.section = section;
                        cx.notify();
                    }))
            })
            .collect::<Vec<_>>();
        div().size_full().flex().bg(rgb(0x0d1720)).text_color(rgb(0xe4eef4)).font_family("sans-serif")
            .child(div().w(px(220.)).h_full().p_4().flex().flex_col().gap_2().bg(rgb(0x111e28))
                .child(div().text_2xl().font_weight(FontWeight::BOLD).py_4().child("OpenKeys"))
                .children(navigation).child(div().mt_6().text_sm().text_color(rgb(0x9ab1bf)).child("GPUI engineering build")))
            .child(div().flex_1().p_8().flex().flex_col().gap_5()
                .child(div().text_2xl().font_weight(FontWeight::BOLD).child(self.section.title()))
                .child(div().text_color(rgb(0xeac77b)).child("LOCKED - no verified device session"))
                .child(self.panel(cx))
                .child(div().p_4().rounded_md().border_1().border_color(rgb(0x344d5d)).child(self.status.clone()))
                .child(div().text_sm().text_color(rgb(0x9ab1bf)).child("Production device runtime and transport integration are incomplete. Use synthetic test data only.")))
    }
}
pub fn run(cx: &mut App) {
    let bounds = Bounds::centered(None, size(px(1080.), px(760.)), cx);
    cx.open_window(
        WindowOptions::default().window_bounds(WindowBounds::Windowed(bounds)),
        |_, cx| {
            cx.new(|_| Manager {
                backend: Backend,
                section: Section::Overview,
                status: "Enter the hardware PIN on your key, never in this application.".into(),
            })
        },
    )
    .expect("Unable to open the OpenKeys window");
    cx.activate(true);
}
