use serde::{Deserialize, Serialize};

pub const APP_ID: &str = "io.github.cosmic-ext-app-switcher";
pub const CONFIG_VERSION: u64 = 1;

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub enum Theme {
    #[default]
    Dark,
    Light,
    Frosted,
    Midnight,
}

#[derive(Debug, Clone)]
pub struct ThemeValues {
    pub bg:            [f32; 4],
    pub selected_bg:   [f32; 4],
    pub corner_radius: f32,
    pub icon_size:     u16,
}

impl Theme {
    pub fn values(&self) -> ThemeValues {
        match self {
            Theme::Dark => ThemeValues {
                bg:            [0.13, 0.13, 0.13, 0.92],
                selected_bg:   [1.0,  1.0,  1.0,  0.25],
                corner_radius: 14.0,
                icon_size:     60,
            },
            Theme::Light => ThemeValues {
                bg:            [0.95, 0.95, 0.95, 0.88],
                selected_bg:   [0.0,  0.0,  0.0,  0.12],
                corner_radius: 14.0,
                icon_size:     60,
            },
            Theme::Frosted => ThemeValues {
                bg:            [0.15, 0.15, 0.15, 0.60],
                selected_bg:   [1.0,  1.0,  1.0,  0.18],
                corner_radius: 18.0,
                icon_size:     60,
            },
            Theme::Midnight => ThemeValues {
                bg:            [0.05, 0.07, 0.15, 0.95],
                selected_bg:   [0.40, 0.60, 1.0,  0.30],
                corner_radius: 14.0,
                icon_size:     60,
            },
        }
    }

    pub fn label(&self) -> &str {
        match self {
            Theme::Dark     => "Dark",
            Theme::Light    => "Light",
            Theme::Frosted  => "Frosted",
            Theme::Midnight => "Midnight",
        }
    }

    pub fn all() -> [Theme; 4] {
        [Theme::Dark, Theme::Light, Theme::Frosted, Theme::Midnight]
    }

    pub fn preview_bg(&self) -> [f32; 3] {
        let v = self.values();
        [v.bg[0], v.bg[1], v.bg[2]]
    }
}
