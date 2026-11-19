//! Lightweight i18n: English + German, system language by default.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    En,
    De,
}

impl Lang {
    pub fn resolve(setting: &str) -> Lang {
        match setting {
            "de" => Lang::De,
            "en" => Lang::En,
            _ => {
                let loc = std::env::var("DRIFTLAB_LANG").ok().or_else(sys_locale::get_locale).unwrap_or_default().to_lowercase();
                if loc.starts_with("de") {
                    Lang::De
                } else {
                    Lang::En
                }
            }
        }
    }
}

/// Translate; falls back to English, then to the key itself.
pub fn tr(lang: Lang, key: &'static str) -> &'static str {
    match TABLE.iter().find(|(k, _, _)| *k == key) {
        Some((_, en, de)) => match lang {
            Lang::En => en,
            Lang::De => {
                if de.is_empty() {
                    en
                } else {
                    de
                }
            }
        },
        None => key,
    }
}

/// (key, English, German)
pub const TABLE: &[(&str, &str, &str)] = &[
    ("nav.controllers", "CONTROLLERS", "CONTROLLER"),
    ("nav.none", "No controller connected", "Kein Controller verbunden"),
    ("nav.overview", "Overview", "Übersicht"),
    ("nav.diagnose", "Drift diagnosis", "Drift-Diagnose"),
    ("nav.wizard", "Calibration", "Kalibrierung"),
    ("nav.tuning", "Software correction", "Software-Korrektur"),
    ("nav.test", "Button test", "Tastentest"),
    ("nav.backups", "Backups", "Backups"),
    ("nav.settings", "Settings", "Einstellungen"),
    ("welcome.detected", "detected", "erkannt"),
    ("start.title", "Connect a controller", "Controller anschließen"),
    ("start.sub", "DriftLab detects controllers automatically – no restart needed.", "DriftLab erkennt Controller automatisch – ohne Neustart."),
    ("start.hint", "Plug in a controller via USB or pair it over Bluetooth", "Controller per USB anschließen oder per Bluetooth koppeln"),
    ("start.usb", "USB is recommended: hardware calibration can only be written over a cable.", "USB wird empfohlen: Hardware-Kalibrierung kann nur per Kabel geschrieben werden."),
    ("start.demo", "Start demo mode", "Demo-Modus starten"),
    ("start.rescan", "Scan again", "Erneut suchen"),
    ("start.scanning", "Scanning for controllers…", "Suche nach Controllern…"),
    ("overview.title", "Overview", "Übersicht"),
    ("overview.viewhint", "Drag to rotate · scroll to zoom · double-click to reset", "Ziehen zum Drehen · Mausrad zum Zoomen · Doppelklick zum Zurücksetzen"),
    ("overview.gyro", "Gyro tilt", "Gyro-Neigung"),
    ("overview.fallback2d", "2D mode", "2D-Modus"),
    ("overview.serial", "Serial / MAC", "Seriennr. / MAC"),
    ("overview.caps", "Capabilities", "Fähigkeiten"),
    ("overview.quick", "Drift quick check", "Drift-Schnellcheck"),
    ("overview.noresult", "No test yet – run a 5-second drift test.", "Noch kein Test – starte einen 5-Sekunden-Drift-Test."),
    ("cap.hw", "Hardware calibration", "Hardware-Kalibrierung"),
    ("cap.sw", "Software correction", "Software-Korrektur"),
    ("cap.triggers", "Analog triggers", "Analoge Trigger"),
    ("cap.gyro", "Gyro", "Gyro"),
    ("cap.touchpad", "Touchpad", "Touchpad"),
    ("cap.rumble", "Vibration", "Vibration"),
    ("cap.lightbar", "Lightbar", "Lightbar"),
    ("btn.drift_test", "Test drift", "Drift testen"),
    ("btn.calibrate", "Calibrate", "Kalibrieren"),
    ("btn.cancel", "Cancel", "Abbrechen"),
    ("btn.next", "Next", "Weiter"),
    ("btn.start", "Start", "Starten"),
    ("btn.begin", "Begin", "Beginnen"),
    ("btn.skip", "Skip", "Überspringen"),
    ("btn.finish", "Finish", "Fertig"),
    ("btn.apply", "Apply", "Anwenden"),
    ("btn.copy_details", "Copy details", "Details kopieren"),
    ("btn.export_csv", "Export CSV", "CSV exportieren"),
    ("btn.export_json", "Export JSON", "JSON exportieren"),
    ("btn.report", "Save report (PNG)", "Bericht speichern (PNG)"),
    ("verdict.ok", "OK", "In Ordnung"),
    ("verdict.slight", "Slight drift", "Leichte Drift"),
    ("verdict.strong", "Strong drift", "Starke Drift"),
    ("verdict.wear", "Hardware wear likely", "Hardware-Verschleiß wahrscheinlich"),
    ("wear.title", "Probably worn hardware – calibration only helps to a limited extent.", "Wahrscheinlich Hardware-Verschleiß – Kalibrierung hilft nur begrenzt."),
    ("wear.body", "The stick potentiometer is noisy or far off-centre. Software can hide part of this, but not repair it. Options:", "Das Potentiometer des Sticks rauscht oder liegt weit neben der Mitte. Software kann einen Teil verdecken, aber nicht reparieren. Optionen:"),
    ("wear.o1", "Manufacturer support / warranty claim", "Herstellersupport bzw. Garantie"),
    ("wear.o2", "Replace the stick module (soldering or plug-in modules)", "Stick-Modul tauschen (Löten oder Steckmodule)"),
    ("wear.o3", "Hall-effect replacement modules (no potentiometer wear)", "Hall-Effekt-Ersatzmodule (kein Potentiometer-Verschleiß)"),
    ("wear.o4", "A professional repair service", "Ein professioneller Reparaturdienst"),
    ("diag.title", "Drift diagnosis", "Drift-Diagnose"),
    ("diag.btn.drift", "Auto drift test (5 s)", "Auto-Drift-Test (5 s)"),
    ("diag.btn.circ", "Circularity test", "Zirkularitätstest"),
    ("diag.btn.trig", "Trigger test", "Trigger-Test"),
    ("diag.corrected", "Show software-corrected values", "Software-korrigierte Werte zeigen"),
    ("diag.left", "Left stick", "Linker Stick"),
    ("diag.right", "Right stick", "Rechter Stick"),
    ("diag.rest.title", "Let go of both sticks", "Lass beide Sticks los"),
    ("diag.rest.body", "Keep your hands off the controller for the whole countdown.", "Nimm während des gesamten Countdowns die Hände vom Controller."),
    ("diag.circ.title", "Rotate each stick 3× slowly along the outer rim", "Dreh jeden Stick 3× langsam am äußeren Rand"),
    ("diag.circ.body", "The ring fills as every direction is covered (95 % needed).", "Der Ring füllt sich, sobald jede Richtung abgedeckt ist (95 % nötig)."),
    ("diag.trig.title", "Press both triggers slowly all the way, then release", "Drück beide Trigger langsam ganz durch und lass los"),
    ("diag.trig.body", "Recording for 6 seconds.", "Aufnahme für 6 Sekunden."),
    ("diag.results", "Drift test result", "Ergebnis Drift-Test"),
    ("diag.score", "Drift score (0–100)", "Drift-Score (0–100)"),
    ("diag.offset", "Centre offset", "Mittelpunkt-Versatz"),
    ("diag.radius", "Offset radius", "Versatz-Radius"),
    ("diag.noise", "Noise (σ)", "Rauschen (σ)"),
    ("diag.jitter", "Jitter (peak-to-peak)", "Jitter (Spitze-Spitze)"),
    ("diag.circ.result", "Circularity", "Zirkularität"),
    ("diag.weakest", "Weakest direction:", "Schwächste Richtung:"),
    ("diag.trig.result", "Trigger test", "Trigger-Test"),
    ("diag.trig.rest", "Rest", "Ruhe"),
    ("diag.trig.max", "Max", "Max"),
    ("diag.trig.lin", "Linearity", "Linearität"),
    ("diag.timeline", "Timeline", "Zeitverlauf"),
    ("toast.saved", "Saved:", "Gespeichert:"),
    ("toast.restored", "Calibration restored.", "Kalibrierung wiederhergestellt."),
    ("toast.profile_saved", "Profile saved.", "Profil gespeichert."),
    ("wiz.title", "Calibration assistant", "Kalibrierungs-Assistent"),
    ("wiz.s.precheck", "Pre-check", "Vorabprüfung"),
    ("wiz.s.backup", "Backup", "Backup"),
    ("wiz.s.center", "Centre", "Mittelpunkt"),
    ("wiz.s.range", "Range", "Bereich"),
    ("wiz.s.trigger", "Triggers", "Trigger"),
    ("wiz.s.compare", "Compare", "Vergleich"),
    ("wiz.s.decide", "Decide", "Entscheiden"),
    ("wiz.s.done", "Done", "Fertig"),
    ("wiz.err.disconnected", "The controller was disconnected – the calibration was safely cancelled.", "Der Controller wurde getrennt – die Kalibrierung wurde sicher abgebrochen."),
    ("wiz.err.nobackup", "The controller returned no readable calibration data.", "Der Controller lieferte keine lesbaren Kalibrierungsdaten."),
    ("wiz.err.hw", "Hardware error", "Hardware-Fehler"),
    ("wiz.err.backup", "Backup failed – falling back to software correction.", "Backup fehlgeschlagen – Wechsel zur Software-Korrektur."),
    ("wiz.pre.title", "Pre-flight checks", "Vorabprüfung"),
    ("wiz.pre.body", "DriftLab checks connection, battery and firmware before anything is written.", "DriftLab prüft Verbindung, Akku und Firmware, bevor etwas geschrieben wird."),
    ("wiz.pre.hw", "Hardware calibration available", "Hardware-Kalibrierung verfügbar"),
    ("wiz.pre.warning", "This writes calibration data into the controller. A failed or interrupted write can make the sticks behave wrongly. A backup is created first, the result is applied temporarily (until the controller restarts) and only saved permanently if you choose so. Do not unplug the cable during the process.", "Dabei werden Kalibrierdaten in den Controller geschrieben. Ein fehlgeschlagener oder unterbrochener Schreibvorgang kann die Sticks fehlerhaft arbeiten lassen. Zuerst wird ein Backup erstellt, das Ergebnis gilt zunächst nur temporär (bis zum Neustart des Controllers) und wird nur auf deinen Wunsch dauerhaft gespeichert. Ziehe während des Vorgangs nicht das Kabel."),
    ("wiz.pre.ack", "I understand the risk", "Ich verstehe das Risiko"),
    ("wiz.pre.sw", "Software correction will be used", "Es wird die Software-Korrektur verwendet"),
    ("wiz.pre.sw_why_blocked", "A safety check blocked hardware writes (see above). Connect via USB, charge the controller or fix the listed issue to enable hardware calibration.", "Eine Sicherheitsprüfung blockiert Hardware-Schreibzugriffe (siehe oben). Per USB verbinden, Controller laden oder das genannte Problem beheben, um die Hardware-Kalibrierung zu aktivieren."),
    ("wiz.pre.sw_why_unsupported", "This controller has no open calibration protocol. DriftLab corrects centre, deadzones and curves in software instead.", "Dieser Controller hat kein offenes Kalibrierungsprotokoll. DriftLab korrigiert stattdessen Mittelpunkt, Deadzones und Kurven per Software."),
    ("wiz.backup.title", "Back up current calibration", "Aktuelle Kalibrierung sichern"),
    ("wiz.backup.body", "All readable calibration data is stored as JSON (with timestamp, serial and firmware).", "Alle auslesbaren Kalibrierdaten werden als JSON gespeichert (mit Zeitstempel, Seriennummer und Firmware)."),
    ("wiz.backup.saved", "Backup saved", "Backup gespeichert"),
    ("wiz.center.title", "Centre point", "Mittelpunkt"),
    ("wiz.center.body", "Let go of both sticks and keep the controller still on a table.", "Lass beide Sticks los und leg den Controller ruhig auf einen Tisch."),
    ("wiz.center.before", "Measuring the current state (before)…", "Messe den aktuellen Zustand (vorher)…"),
    ("wiz.center.rounds", "Capturing the centre in 3 rounds…", "Erfasse den Mittelpunkt in 3 Runden…"),
    ("wiz.center.moved", "Movement detected – restarting. Please don't touch the sticks.", "Bewegung erkannt – Neustart. Bitte die Sticks nicht berühren."),
    ("wiz.working", "Working…", "Arbeite…"),
    ("wiz.range.title", "Range / rim", "Bereich / Rand"),
    ("wiz.range.body", "Circle each stick several times along the outer rim until the ring is full.", "Kreise jeden Stick mehrfach am äußeren Rand, bis der Ring voll ist."),
    ("wiz.trig.title", "Triggers", "Trigger"),
    ("wiz.trig.body", "Release, then press both triggers fully. (The open protocol does not allow trigger calibration; the values are checked only.)", "Loslassen, dann beide Trigger ganz durchdrücken. (Das offene Protokoll erlaubt keine Trigger-Kalibrierung; die Werte werden nur geprüft.)"),
    ("wiz.compare.title", "Before / after comparison", "Vorher/Nachher-Vergleich"),
    ("wiz.compare.body_hw", "The calibration is applied temporarily. Let go of the sticks for another drift test.", "Die Kalibrierung ist temporär angewendet. Lass die Sticks für einen erneuten Drift-Test los."),
    ("wiz.compare.body_sw", "The software correction is simulated. Let go of the sticks for another drift test.", "Die Software-Korrektur wird simuliert. Lass die Sticks für einen erneuten Drift-Test los."),
    ("wiz.compare.before", "Before", "Vorher"),
    ("wiz.compare.after", "After", "Nachher"),
    ("wiz.compare.improved", "Measurable improvement (score change shown above).", "Messbare Verbesserung (Score-Änderung oben)."),
    ("wiz.compare.notimproved", "The measurement does not show a clear improvement.", "Die Messung zeigt keine eindeutige Verbesserung."),
    ("wiz.decide.title", "Keep the result?", "Ergebnis übernehmen?"),
    ("wiz.decide.body_hw", "Save permanently writes to the controller. Discard returns to the previous state (power-cycle the controller if it still behaves differently).", "Dauerhaft speichern schreibt in den Controller. Verwerfen stellt den vorherigen Zustand wieder her (Controller aus- und einschalten, falls er sich noch anders verhält)."),
    ("wiz.decide.body_sw", "Save stores the correction in this controller's profile.", "Speichern legt die Korrektur im Profil dieses Controllers ab."),
    ("wiz.decide.warn_noimp", "No clear improvement was measured – saving is not recommended.", "Es wurde keine klare Verbesserung gemessen – Speichern wird nicht empfohlen."),
    ("wiz.decide.save_hw", "Save permanently", "Dauerhaft speichern"),
    ("wiz.decide.save_sw", "Save profile", "Profil speichern"),
    ("wiz.decide.discard", "Discard", "Verwerfen"),
    ("wiz.decide.discard_hint", "Discarded. Power-cycle the controller to be sure the temporary calibration is gone.", "Verworfen. Schalte den Controller aus und wieder ein, damit die temporäre Kalibrierung sicher verschwindet."),
    ("wiz.decide.repeat", "Repeat", "Wiederholen"),
    ("wiz.done.good", "Drift measurably reduced", "Drift messbar reduziert"),
    ("wiz.done.honest", "No measurable improvement", "Keine messbare Verbesserung"),
    ("wiz.done.good_body", "The after-test confirms a lower drift score.", "Der Nachher-Test bestätigt einen geringeren Drift-Score."),
    ("wiz.done.honest_body", "Calibration could not fix the drift – this usually means worn hardware.", "Die Kalibrierung konnte die Drift nicht beheben – meist ist die Hardware verschlissen."),
    ("wiz.done.saved_hw", "The calibration was saved permanently.", "Die Kalibrierung wurde dauerhaft gespeichert."),
    ("wiz.done.temp", "The calibration is temporary and ends when the controller restarts.", "Die Kalibrierung ist temporär und endet beim Neustart des Controllers."),
    ("tune.title", "Software correction", "Software-Korrektur"),
    ("tune.sub", "Works for every controller, including Xbox. Applied per controller profile.", "Funktioniert für jeden Controller, auch Xbox. Wird pro Controller-Profil angewendet."),
    ("tune.new", "New profile", "Neues Profil"),
    ("tune.delete", "Delete", "Löschen"),
    ("tune.export", "Export…", "Exportieren…"),
    ("tune.import", "Import…", "Importieren…"),
    ("tune.name", "Name", "Name"),
    ("tune.settings", "Settings", "Einstellungen"),
    ("tune.cx", "Centre X", "Mitte X"),
    ("tune.cy", "Centre Y", "Mitte Y"),
    ("tune.kind", "Deadzone type", "Deadzone-Typ"),
    ("tune.inner", "Inner deadzone", "Innere Deadzone"),
    ("tune.outer", "Outer deadzone", "Äußere Deadzone"),
    ("tune.anti", "Anti-deadzone", "Anti-Deadzone"),
    ("tune.circ", "Circularity gain", "Zirkularitäts-Faktor"),
    ("tune.curve", "Response curve", "Response-Kurve"),
    ("curve.linear", "Linear", "Linear"),
    ("curve.exp", "Exponential", "Exponentiell"),
    ("curve.s", "S-curve", "S-Kurve"),
    ("curve.bezier", "Custom Bézier", "Eigene Bézier-Kurve"),
    ("tune.strength", "Strength", "Stärke"),
    ("tune.take_offset", "Use offset from drift test", "Versatz aus Drift-Test übernehmen"),
    ("tune.take_circ", "Use circularity from test", "Zirkularität aus Test übernehmen"),
    ("tune.auto", "Optimise automatically", "Automatisch optimieren"),
    ("tune.auto_hint", "Run a drift test first to enable automatic optimisation.", "Starte zuerst einen Drift-Test, um die Auto-Optimierung zu nutzen."),
    ("tune.preview", "Live preview", "Live-Vorschau"),
    ("tune.input", "Input", "Eingabe"),
    ("tune.output", "Output", "Ausgabe"),
    ("tune.response", "Response (input → output)", "Antwort (Eingabe → Ausgabe)"),
    ("tune.system", "System-wide correction", "Systemweite Korrektur"),
    ("tune.system_toggle", "Apply correction system-wide (virtual controller)", "Korrektur systemweit anwenden (virtueller Controller)"),
    ("tune.system_note_win", "Not available in v0.1.0: the common virtual-controller driver for Windows (ViGEmBus) is no longer maintained, and no clean replacement exists. Use the Steam Input export below.", "In v0.1.0 nicht verfügbar: Der gängige Treiber für virtuelle Controller unter Windows (ViGEmBus) wird nicht mehr gepflegt, ein sauberer Ersatz existiert nicht. Nutze den Steam-Input-Export unten."),
    ("tune.system_note_linux", "Not yet available in v0.1.0 (a uinput virtual controller is on the roadmap). Use the Steam Input export below.", "In v0.1.0 noch nicht verfügbar (ein uinput-Controller steht auf der Roadmap). Nutze den Steam-Input-Export unten."),
    ("tune.system_note_mac", "No maintained virtual-controller driver exists for macOS. Use the Steam Input export below.", "Für macOS gibt es keinen gepflegten Treiber für virtuelle Controller. Nutze den Steam-Input-Export unten."),
    ("tune.steam", "Export for Steam Input", "Für Steam Input exportieren"),
    ("test.title", "Button test", "Tastentest"),
    ("test.buttons", "Buttons & triggers", "Tasten & Trigger"),
    ("test.touchpad", "Touchpad", "Touchpad"),
    ("test.motion", "Motion sensors", "Bewegungssensoren"),
    ("test.gyro", "Gyro", "Gyro"),
    ("test.accel", "Acceleration", "Beschleunigung"),
    ("test.rumble", "Vibration", "Vibration"),
    ("test.intensity", "Intensity", "Intensität"),
    ("test.left", "Left", "Links"),
    ("test.right", "Right", "Rechts"),
    ("test.both", "Both", "Beide"),
    ("test.unsupported", "Not supported by this controller or connection.", "Von diesem Controller oder dieser Verbindung nicht unterstützt."),
    ("test.lightbar", "Lightbar colour", "Lightbar-Farbe"),
    ("bk.title", "Calibration backups", "Kalibrierungs-Backups"),
    ("bk.sub", "Created automatically before every hardware calibration.", "Werden vor jeder Hardware-Kalibrierung automatisch erstellt."),
    ("bk.empty", "No backups yet.", "Noch keine Backups."),
    ("bk.confirm_delete", "Yes, delete", "Ja, löschen"),
    ("bk.confirm_restore", "Yes, restore", "Ja, wiederherstellen"),
    ("bk.delete", "Delete", "Löschen"),
    ("bk.export", "Export", "Exportieren"),
    ("bk.restore", "Restore", "Wiederherstellen"),
    ("bk.restore_note", "Restoring needs the same controller family, connected via USB.", "Zum Wiederherstellen muss die gleiche Controller-Familie per USB verbunden sein."),
    ("set.title", "Settings", "Einstellungen"),
    ("set.general", "General", "Allgemein"),
    ("set.language", "Language", "Sprache"),
    ("set.auto", "Automatic", "Automatisch"),
    ("set.theme", "Theme", "Theme"),
    ("set.reduced", "Reduced motion", "Reduzierte Bewegung"),
    ("set.scale", "UI scale", "UI-Skalierung"),
    ("set.polling", "Max. polling rate", "Max. Abfragerate"),
    ("set.expert", "Expert mode", "Expertenmodus"),
    ("set.expert_hint", "Enables the HID inspector and hardware writes with unknown firmware", "Aktiviert den HID-Inspektor und Schreibzugriff bei unbekannter Firmware"),
    ("set.maintenance", "Logs & updates", "Logs & Updates"),
    ("set.open_logs", "Open log folder", "Log-Ordner öffnen"),
    ("set.export_logs", "Export logs", "Logs exportieren"),
    ("set.check_updates", "Check for updates", "Nach Updates suchen"),
    ("set.update_available", "Update available:", "Update verfügbar:"),
    ("set.open_download", "Open download page", "Download-Seite öffnen"),
    ("set.up_to_date", "You are up to date.", "Du bist auf dem neuesten Stand."),
    ("set.update_failed", "Update check failed:", "Update-Prüfung fehlgeschlagen:"),
    ("set.update_note", "DriftLab never updates itself silently; it only asks GitHub for the latest release when you click.", "DriftLab aktualisiert sich nie stillschweigend; es fragt nur auf Klick GitHub nach dem neuesten Release."),
    ("set.inspector", "HID report inspector", "HID-Report-Inspektor"),
    ("set.inspector_hint", "Live hex view of the last input report with field highlighting.", "Live-Hex-Ansicht des letzten Input-Reports mit Feld-Hervorhebung."),
    ("set.no_device", "No controller selected.", "Kein Controller gewählt."),
    ("set.about", "About", "Über"),
    ("set.no_logs", "No log files yet.", "Noch keine Log-Dateien."),
    ("about.disclaimer", "Use at your own risk. Hardware calibration writes to your controller; no warranty of any kind.", "Nutzung auf eigene Gefahr. Die Hardware-Kalibrierung schreibt in deinen Controller; keinerlei Gewährleistung."),
    ("about.trademark", "DriftLab is not affiliated with, endorsed or sponsored by Sony Interactive Entertainment or Microsoft. All trademarks belong to their respective owners.", "DriftLab steht in keiner Verbindung zu Sony Interactive Entertainment oder Microsoft und wird von diesen weder unterstützt noch gesponsert. Alle Marken gehören ihren jeweiligen Inhabern."),
    ("about.credits", "Credits", "Danksagung"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn german_translations_complete() {
        for (k, en, de) in TABLE {
            assert!(!en.is_empty(), "{k}: empty English");
            assert!(!de.is_empty(), "{k}: missing German");
        }
    }

    #[test]
    fn keys_unique() {
        let mut seen = std::collections::HashSet::new();
        for (k, _, _) in TABLE {
            assert!(seen.insert(*k), "duplicate key {k}");
        }
    }

    /// Every `t("key")` / `tr(lang, "key")` literal used in the sources must exist in the table.
    #[test]
    fn all_used_keys_exist() {
        fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, out);
                } else if p.extension().is_some_and(|x| x == "rs") {
                    out.push(p);
                }
            }
        }
        let mut files = vec![];
        walk(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut files);
        let mut missing = vec![];
        for f in files {
            if f.ends_with("i18n.rs") {
                continue;
            }
            let src = std::fs::read_to_string(&f).unwrap_or_default();
            for pat in ["t(\"", "tr(app.lang(), \"", "\""] {
                if pat == "\"" {
                    continue;
                }
                let mut rest = src.as_str();
                while let Some(i) = rest.find(pat) {
                    let after = &rest[i + pat.len()..];
                    if let Some(j) = after.find('"') {
                        let key = &after[..j];
                        // Only dotted lowercase identifiers are keys.
                        let is_key =
                            key.contains('.') && key.chars().all(|c| c.is_ascii_lowercase() || c == '.' || c == '_' || c.is_ascii_digit());
                        let before = &rest[..i];
                        let call_like = before.ends_with('.') || before.ends_with("tr(app.lang(), ") || pat.starts_with("tr");
                        if is_key && call_like && !TABLE.iter().any(|(k, _, _)| k == &key) {
                            missing.push(format!("{}: {key}", f.display()));
                        }
                    }
                    rest = &rest[i + pat.len()..];
                }
            }
        }
        assert!(missing.is_empty(), "missing keys: {missing:#?}");
    }
}
