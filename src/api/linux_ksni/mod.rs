use crate::{IconSource, TIError};
use ksni::{menu::StandardItem, Handle, Icon};
use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

type CallBackEntry = Option<Box<dyn Fn() + Send + 'static>>;

enum TrayItem {
    Label(String),
    MenuItem {
        id: u32,
        label: String,
        action: Arc<dyn Fn() + Send + Sync + 'static>,
    },
    Separator,
}

struct Tray {
    title: String,
    icon: IconSource,
    actions: Vec<TrayItem>,
    next_id: u32,
    last_click: Option<Instant>,
    icon_click_cb: Arc<Mutex<CallBackEntry>>,
    icon_double_click_cb: Arc<Mutex<CallBackEntry>>,
}

pub struct TrayItemLinux {
    tray: Handle<Tray>,
}

impl ksni::Tray for Tray {
    fn activate(&mut self, _x: i32, _y: i32) {
        padlock::mutex_lock(&self.icon_click_cb, |cb| match cb {
            Some(f) => f(),
            None => (),
        });
        if let Some(last_click) = self.last_click {
            self.last_click = None;
            if last_click.elapsed().as_millis() <= 150 {
                padlock::mutex_lock(&self.icon_double_click_cb, |cb| match cb {
                    Some(f) => f(),
                    None => (),
                });
            }
            return;
        }
        self.last_click = Some(Instant::now())
    }

    fn id(&self) -> String {
        self.title.clone()
    }

    fn title(&self) -> String {
        self.title.clone()
    }

    fn icon_name(&self) -> String {
        match &self.icon {
            IconSource::Resource(name) => name.to_string(),
            IconSource::Data { .. } => String::new(),
        }
    }

    fn icon_pixmap(&self) -> Vec<Icon> {
        match &self.icon {
            IconSource::Resource(_) => vec![],
            IconSource::Data {
                data,
                height,
                width,
            } => {
                vec![Icon {
                    width: *height,
                    height: *width,
                    data: data.clone(),
                }]
            }
        }
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        self.actions
            .iter()
            .map(|item| match item {
                TrayItem::Label(label) => StandardItem {
                    label: label.clone(),
                    enabled: false,
                    ..Default::default()
                }
                .into(),
                TrayItem::MenuItem { label, action, .. } => {
                    let action = action.clone();
                    StandardItem {
                        label: label.clone(),
                        activate: Box::new(move |_| {
                            action();
                        }),
                        ..Default::default()
                    }
                    .into()
                }
                TrayItem::Separator => ksni::MenuItem::Separator,
            })
            .collect()
    }
}

impl TrayItemLinux {
    pub fn new(title: &str, icon: IconSource) -> Result<Self, TIError> {
        let icon_click_cb = Arc::new(Mutex::new(None));
        let icon_double_click_cb = Arc::new(Mutex::new(None));

        let svc = ksni::TrayService::new(Tray {
            title: title.to_string(),
            icon,
            actions: vec![],
            next_id: 0,
            last_click: None,
            icon_click_cb,
            icon_double_click_cb,
        });

        let handle = svc.handle();
        svc.spawn();

        Ok(Self { tray: handle })
    }

    pub fn set_icon(&mut self, icon: IconSource) -> Result<(), TIError> {
        self.tray.update(|tray| tray.icon = icon.clone());

        Ok(())
    }

    pub fn add_label(&mut self, label: &str) -> Result<(), TIError> {
        self.tray.update(move |tray| {
            tray.actions.push(TrayItem::Label(label.to_string()));
        });

        Ok(())
    }

    pub fn set_icon_click_callback<F>(&mut self, cb: F) -> Result<(), TIError>
    where
        F: Fn() + Send + 'static,
    {
        self.tray.update(|tray| {
            padlock::mutex_lock(&tray.icon_click_cb, |icon_click_cb| {
                *icon_click_cb = Some(Box::new(cb))
            })
        });
        Ok(())
    }

    pub fn set_icon_double_click_callback<F>(&mut self, cb: F) -> Result<(), TIError>
    where
        F: Fn() + Send + 'static,
    {
        self.tray.update(|tray| {
            padlock::mutex_lock(&tray.icon_double_click_cb, |icon_double_click_cb| {
                *icon_double_click_cb = Some(Box::new(cb))
            })
        });
        Ok(())
    }

    pub fn add_menu_item<F>(&mut self, label: &str, cb: F) -> Result<(), TIError>
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.add_menu_item_with_id(label, cb)?;
        Ok(())
    }

    pub fn add_menu_item_with_id<F>(&mut self, label: &str, cb: F) -> Result<u32, TIError>
    where
        F: Fn() + Send + Sync + 'static,
    {
        let action = Arc::new(cb);
        let item_id = Arc::new(Mutex::new(0));
        let item_id_clone = Arc::clone(&item_id);

        self.tray.update(move |tray| {
            let mut id = item_id_clone.lock().unwrap();
            *id = tray.next_id;
            tray.next_id += 1;

            tray.actions.push(TrayItem::MenuItem {
                id: *id,
                label: label.to_string(),
                action: action.clone(),
            });
        });

        let final_id = *item_id.lock().unwrap();
        Ok(final_id)
    }

    pub fn set_menu_item_label(&mut self, label: &str, id: u32) -> Result<(), TIError> {
        self.tray.update(move |tray| {
            if let Some(item) = tray.actions.iter_mut().find_map(|item| match item {
                TrayItem::MenuItem {
                    id: item_id, label, ..
                } if *item_id == id => Some(label),
                _ => None,
            }) {
                *item = label.to_string();
            }
        });

        Ok(())
    }

    pub fn add_separator(&mut self) -> Result<(), TIError> {
        self.tray.update(move |tray| {
            tray.actions.push(TrayItem::Separator);
        });

        Ok(())
    }
}
