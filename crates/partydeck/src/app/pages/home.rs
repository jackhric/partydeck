use eframe::egui::Ui;

const KOFI_URL: &str = "https://ko-fi.com/wunner";
const GITHUB_URL: &str = "https://github.com/wunnr/partydeck";
const SUPPORTERS: &str = "Framilano, Jayden, Marc, Max Rei";

const CONTRIBUTORS: &[(&str, &str)] = &[
    ("@Blahkaey", "https://github.com/Blahkaey"),
    ("@blckink", "https://github.com/blckink"),
    ("@cseelhoff", "https://github.com/cseelhoff"),
    ("@davidawesome-02", "https://github.com/davidawesome-02"),
    ("@felipecrs", "https://github.com/felipecrs"),
    ("@framilano", "https://github.com/framilano"),
    ("@FrancisBernard34", "https://github.com/FrancisBernard34"),
    ("@Rudicito", "https://github.com/Rudicito"),
    ("@Tau5", "https://github.com/Tau5"),
    ("@Twig6943", "https://github.com/Twig6943"),
];

pub(super) fn show(ui: &mut Ui) {
    ui.heading("Welcome to PartyDeck");
    ui.separator();
    ui.label("Press SELECT/BACK or Tab to unlock gamepad navigation.");
    ui.label("PartyDeck is in the very early stages of development; as such, you will likely encounter bugs, issues, and strange design decisions.");
    ui.label("For debugging purposes, it's recommended to read terminal output (stdout) for further information on errors.");
    ui.separator();
    ui.horizontal_wrapped(|ui| {
        ui.label("Thank you to");
        ui.hyperlink_to("♥Ko-fi", KOFI_URL);
        ui.label("supporters:");
    });
    ui.label(SUPPORTERS);
    ui.horizontal_wrapped(|ui| {
        ui.label("Thank you to");
        ui.hyperlink_to(" GitHub", GITHUB_URL);
        ui.label("contributors/handler creators:")
    });
    ui.horizontal_wrapped(|ui| {
        for (name, url) in CONTRIBUTORS {
            ui.hyperlink_to(*name, *url);
        }
    });
}
