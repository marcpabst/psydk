use std::borrow::Cow;

pub struct ListView<'a, T: ListItem> {
    items: &'a Vec<T>,
    selected: Option<&'a mut usize>,
}

pub enum ModalType {
    None,
    NewSubject,
    NewSession,
}

impl<'a, T> ListView<'a, T>
where
    T: ListItem,
{
    pub fn new(items: &'a Vec<T>, selected: Option<&'a mut usize>) -> Self {
        Self {
            items,
            selected: selected,
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            for (mut i, item) in self.items.iter().enumerate() {
                let _fill = if self.selected == Some(&mut i) {
                    egui::Color32::from_rgb(0, 140, 255)
                } else {
                    // transparent
                    egui::Color32::TRANSPARENT
                };

                let _text_color = if self.selected == Some(&mut i) {
                    ui.visuals().widgets.active.text_color()
                } else {
                    ui.visuals().widgets.inactive.text_color()
                };

                let _height = item.subtitle().map_or(15.0, |_| 30.0); // default height is 15.0, but if there is a subtitle, it is 30.0

                let response = egui::Frame::NONE
                    .inner_margin(egui::Margin::symmetric(10, 10))
                    .fill(_fill)
                    .show(ui, |ui| {
                        ui.vertical(|ui| {
                            ui.set_width(ui.available_width());
                            ui.set_height(_height);
                            ui.style_mut().interaction.selectable_labels = false;

                            let title = egui::RichText::new(item.title())
                                .color(_text_color)
                                .strong();
                            if let Some(subtitle) = item.subtitle() {
                                ui.label(title);
                                ui.label(
                                    egui::RichText::new(subtitle).color(ui.visuals().text_color()),
                                );
                            } else {
                                ui.label(title);
                            }
                        })
                    })
                    .response
                    .interact(egui::Sense::click());
                if response.clicked() {
                    println!("Clicked on item: {}", item.title());
                    // update the selected index if it exists
                    if let Some(selected) = self.selected.as_mut() {
                        // set index
                        **selected = i;
                    }
                }
            }
        });
    }
}

pub trait ListItem {
    fn title(&self) -> Cow<str>;
    fn subtitle(&self) -> Option<Cow<str>> {
        None
    }
}

impl ListItem for String {
    fn title(&self) -> Cow<str> {
        self.into()
    }

    fn subtitle(&self) -> Option<Cow<str>> {
        None
    }
}
