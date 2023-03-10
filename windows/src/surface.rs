use std::path;
use winit::{
    dpi::{PhysicalPosition, PhysicalSize},
    monitor,
};

#[derive(Clone, Debug, PartialEq)]
pub struct Surface {
    pub position: PhysicalPosition<i32>,
    pub size: PhysicalSize<u32>,
    pub scale_factor: f64,
    pub wallpaper: Option<path::PathBuf>,
}

impl Surface {
    fn from_monitor(monitor: &monitor::MonitorHandle, wallpaper: &Option<path::PathBuf>) -> Self {
        Self {
            position: monitor.position(),
            size: monitor.size(),
            scale_factor: monitor.scale_factor(),
            wallpaper: wallpaper.clone(),
        }
    }

    fn merge(&mut self, surface: &Self) {
        // if self.scale_factor != surface.scale_factor {
        //     return None;
        // }

        self.position = PhysicalPosition::new(
            self.position.x.min(surface.position.x),
            self.position.y.min(surface.position.y),
        );
        self.size = PhysicalSize::new(
            self.size.width + surface.size.width,
            self.size.height + surface.size.height,
        );
    }
}

pub fn combine_monitors(
    monitors: &[(monitor::MonitorHandle, Option<path::PathBuf>)],
) -> Vec<Surface> {
    use std::collections::HashMap;

    let mut grouped_by_size: HashMap<PhysicalSize<u32>, Surface> = HashMap::new();
    for monitor in monitors.iter() {
        let surface = Surface::from_monitor(&monitor.0, &monitor.1);
        grouped_by_size
            .entry(monitor.0.size())
            .and_modify(|existing_surface| existing_surface.merge(&surface))
            .or_insert(surface);
    }

    grouped_by_size.into_values().collect::<Vec<Surface>>()
}

// #[cfg(test)]
// mod test {
//     use super::*;
//
//     #[test]
//     fn it_does_not_combine_two_different_displays() {
//         let display0 = Surface::from_bounds(Rect::new(0, 0, 3360, 2100), BASE_DPI as f64);
//         let display1 = Surface::from_bounds(
//             Rect::new(display0.bounds.width() as i32, 0, 2560, 1440),
//             BASE_DPI as f64,
//         );
//
//         assert_eq!(
//             Surface::combine_displays(&[display0, display1]),
//             vec![display0, display1]
//         );
//     }
//
//     #[test]
//     fn it_partially_combines_two_1440p_displays_and_a_separate_laptop_display() {
//         // 1440p + 1440p + laptop
//         let display0 = Surface::from_bounds(Rect::new(-2560, 0, 2560, 1440), BASE_DPI as f64);
//         let display1 = Surface::from_bounds(Rect::new(0, 0, 2560, 1440), BASE_DPI as f64);
//         let display2 = Surface::from_bounds(Rect::new(2560, 0, 3360, 2100), BASE_DPI as f64);
//
//         assert_eq!(
//             Surface::combine_displays(&[display0, display1, display2]),
//             vec![
//                 Surface::from_bounds(Rect::new(-2560, 0, 5120, 1440), BASE_DPI as f64),
//                 display2
//             ]
//         );
//
//         // laptop + 1440p + 1440p
//         let display2 = Surface::from_bounds(Rect::new(-1920, 360, 1920, 1080), BASE_DPI as f64);
//         let display0 = Surface::from_bounds(Rect::new(0, 0, 2560, 1440), BASE_DPI as f64);
//         let display1 = Surface::from_bounds(Rect::new(2560, 0, 2560, 1440), BASE_DPI as f64);
//
//         assert_eq!(
//             Surface::combine_displays(&[display2, display0, display1]),
//             vec![
//                 display2,
//                 Surface::from_bounds(Rect::new(0, 0, 5120, 1440), BASE_DPI as f64),
//             ]
//         );
//     }
//
//     #[test]
//     fn it_combines_two_1440p_displays() {
//         let display0 = Surface::from_bounds(Rect::new(0, 0, 2560, 1440), BASE_DPI as f64);
//         let display1 = Surface::from_bounds(
//             Rect::new(display0.bounds.width() as i32, 0, 2560, 1440),
//             BASE_DPI as f64,
//         );
//
//         assert_eq!(
//             Surface::combine_displays(&[display0, display1]),
//             vec![Surface::from_bounds(
//                 Rect::new(0, 0, 5120, 1440),
//                 BASE_DPI as f64
//             )]
//         );
//     }
//
//     #[test]
//     fn it_combines_three_1440p_displays() {
//         let display0 = Surface::from_bounds(Rect::new(-2560, 0, 2560, 1440), BASE_DPI as f64);
//         let display1 = Surface::from_bounds(Rect::new(0, 0, 2560, 1440), BASE_DPI as f64);
//         let display2 = Surface::from_bounds(Rect::new(2560, 0, 2560, 1440), BASE_DPI as f64);
//
//         assert_eq!(
//             Surface::combine_displays(&[display0, display1, display2]),
//             vec![Surface::from_bounds(
//                 Rect::new(-2560, 0, 2560 * 3, 1440),
//                 BASE_DPI as f64
//             )]
//         );
//     }
//
//     #[test]
//     fn it_combines_a_grid_of_displays() {
//         let display0 = Surface::from_bounds(Rect::new(0, 0, 2560, 1440), BASE_DPI as f64);
//         let display1 = Surface::from_bounds(Rect::new(2560, 0, 2560, 1440), BASE_DPI as f64);
//         let display2 = Surface::from_bounds(Rect::new(0, 1440, 2560, 1440), BASE_DPI as f64);
//         let display3 = Surface::from_bounds(Rect::new(2560, 1440, 2560, 1440), BASE_DPI as f64);
//
//         assert_eq!(
//             Surface::combine_displays(&[display0, display1, display2, display3]),
//             vec![Surface::from_bounds(
//                 Rect::new(0, 0, 2560 * 2, 1440 * 2),
//                 BASE_DPI as f64
//             ),]
//         );
//
//         let laptop = Surface::from_bounds(Rect::new(2560 * 2, 0, 1920, 1080), BASE_DPI as f64);
//         assert_eq!(
//             Surface::combine_displays(&[display0, display1, display2, display3, laptop]),
//             vec![
//                 Surface::from_bounds(Rect::new(0, 0, 2560 * 2, 1440 * 2), BASE_DPI as f64),
//                 laptop
//             ]
//         );
//     }
// }
