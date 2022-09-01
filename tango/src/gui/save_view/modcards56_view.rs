use fluent_templates::Loader;

use crate::{game, gui, i18n, rom, save};

pub struct State {}

impl State {
    pub fn new() -> Self {
        Self {}
    }
}

fn show_effect(ui: &mut egui::Ui, name: egui::RichText, is_enabled: bool, is_debuff: bool) {
    egui::Frame::none()
        .inner_margin(egui::style::Margin::symmetric(4.0, 0.0))
        .rounding(egui::Rounding::same(2.0))
        .fill(if is_enabled {
            if is_debuff {
                egui::Color32::from_rgb(0xb5, 0x5a, 0xde)
            } else {
                egui::Color32::from_rgb(0xff, 0xbd, 0x18)
            }
        } else {
            egui::Color32::from_rgb(0xbd, 0xbd, 0xbd)
        })
        .show(ui, |ui| {
            ui.label(name.color(egui::Color32::BLACK));
        });
}

pub struct Modcards56View {}

impl Modcards56View {
    pub fn new() -> Self {
        Self {}
    }

    pub fn show<'a>(
        &mut self,
        ui: &mut egui::Ui,
        clipboard: &mut arboard::Clipboard,
        font_families: &gui::FontFamilies,
        lang: &unic_langid::LanguageIdentifier,
        game: &'static (dyn game::Game + Send + Sync),
        modcards56_view: &Box<dyn save::Modcards56View<'a> + 'a>,
        assets: &Box<dyn rom::Assets + Send + Sync>,
        _state: &mut State,
    ) {
        let items = (0..modcards56_view.count())
            .map(|slot| {
                let modcard = modcards56_view.modcard(slot);
                let effects = modcard
                    .as_ref()
                    .and_then(|item| assets.modcard56(item.id))
                    .map(|info| info.effects.as_slice())
                    .unwrap_or(&[][..]);
                (modcard, effects)
            })
            .collect::<Vec<_>>();

        ui.horizontal(|ui| {
            if ui
                .button(format!(
                    "📋 {}",
                    i18n::LOCALES.lookup(lang, "copy-to-clipboard").unwrap(),
                ))
                .clicked()
            {
                let _ = clipboard.set_text(
                    items
                        .iter()
                        .flat_map(|(modcard, _)| {
                            let modcard = if let Some(modcard) = modcard.as_ref() {
                                modcard
                            } else {
                                return vec![];
                            };

                            if !modcard.enabled {
                                return vec![];
                            }

                            let modcard = if let Some(modcard) = assets.modcard56(modcard.id) {
                                modcard
                            } else {
                                return vec![];
                            };

                            vec![format!("{}\t{}", modcard.name, modcard.mb)]
                        })
                        .collect::<Vec<_>>()
                        .join("\n"),
                );
            }
        });

        egui_extras::TableBuilder::new(ui)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(egui_extras::Size::remainder())
            .column(egui_extras::Size::exact(150.0))
            .column(egui_extras::Size::exact(150.0))
            .striped(true)
            .body(|body| {
                body.heterogeneous_rows(
                    items.iter().map(|(_, effects)| {
                        std::cmp::max(
                            effects.iter().filter(|effect| effect.is_ability).count(),
                            effects.iter().filter(|effect| !effect.is_ability).count(),
                        ) as f32
                            * 20.0
                    }),
                    |i, mut row| {
                        let (modcard, effects) = &items[i];
                        row.col(|ui| {
                            if let Some(modcard) = modcard
                                .as_ref()
                                .and_then(|modcard| assets.modcard56(modcard.id))
                            {
                                ui.vertical(|ui| {
                                    ui.label(
                                        egui::RichText::new(&modcard.name)
                                            .family(font_families.for_language(&game.language())),
                                    );
                                    ui.small(format!("{}MB", modcard.mb));
                                });
                            }
                        });

                        row.col(|ui| {
                            ui.vertical(|ui| {
                                ui.with_layout(
                                    egui::Layout::top_down_justified(egui::Align::Min),
                                    |ui| {
                                        for effect in *effects {
                                            if effect.is_ability {
                                                continue;
                                            }

                                            show_effect(
                                                ui,
                                                egui::RichText::new(&effect.name).family(
                                                    font_families.for_language(&game.language()),
                                                ),
                                                modcard
                                                    .as_ref()
                                                    .map(|modcard| modcard.enabled)
                                                    .unwrap_or(false),
                                                effect.is_debuff,
                                            );
                                        }
                                    },
                                );
                            });
                        });
                        row.col(|ui| {
                            ui.vertical(|ui| {
                                ui.with_layout(
                                    egui::Layout::top_down_justified(egui::Align::Min),
                                    |ui| {
                                        for effect in *effects {
                                            if !effect.is_ability {
                                                continue;
                                            }

                                            show_effect(
                                                ui,
                                                egui::RichText::new(&effect.name).family(
                                                    font_families.for_language(&game.language()),
                                                ),
                                                modcard
                                                    .as_ref()
                                                    .map(|modcard| modcard.enabled)
                                                    .unwrap_or(false),
                                                effect.is_debuff,
                                            );
                                        }
                                    },
                                );
                            });
                        });
                    },
                );
            });
    }
}