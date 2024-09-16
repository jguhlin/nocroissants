#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use egui::{Painter, Pos2, Rect, Sense, Vec2};

// Latitude and Longitude constants
// Lat is -90 to 90
// Lon is -180 to 180

// Let's start at Otago
const LAT: f64 = -45.8667;
const LON: f64 = 170.6667;

// mpsc
use std::sync::mpsc;

enum Message {
    Dummy,
}

enum Command {
    Dates(f64, f64),
    Subset(f64, f64, String), // band is Lai_500m
}

// When compiling natively:
#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    env_logger::init();

    // mpsc for data
    let (tx, rx) = mpsc::channel();
    let (tx_cmd, rx_cmd) = mpsc::channel();

    // Send a command to get the dates
    tx_cmd.send(Command::Dates(LAT, LON)).unwrap();

    // Start a new thread for tokio
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            while let Ok(cmd) = rx_cmd.recv() {
                match cmd {
                    Command::Dates(lat, lon) => {
                        let dates = dates("MCD15A2H", lat, lon).await.unwrap();
                        match tx.send(Message::Dates(dates)) {
                            Ok(_) => (),
                            Err(e) => eprintln!("Error: {}", e),
                        }
                    }
                    Command::Subset(lat, lon, date) => {
                        let subset = subset("MCD15A2H", lat, lon, "Lai_500m", &date, &date, 10, 10)
                            .await
                            .unwrap();
                        println!("{:?}", subset);
                        tx.send(Message::Data(subset)).unwrap();
                    }
                }
            }
        });
    });

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([400.0, 300.0])
            .with_min_inner_size([300.0, 220.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Leaf Area Index",
        native_options,
        Box::new(|cc| Ok(Box::new(LaiApp::new(cc, rx, tx_cmd)))),
    )
}

pub struct LaiApp {
    // rx
    rx: mpsc::Receiver<Message>,
    tx_cmd: mpsc::Sender<Command>,

    dates: Option<DatesWrapper>,
    selected_date: Option<DateInfo>,

    modis_data: Option<ModisData>,
}

impl Default for LaiApp {
    fn default() -> Self {
        Self {
            rx: mpsc::channel().1,
            tx_cmd: mpsc::channel().0,

            dates: None,
            selected_date: None,
            modis_data: None,
        }
    }
}

impl LaiApp {
    /// Called once before the first frame.
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        rx: mpsc::Receiver<Message>,
        tx_cmd: mpsc::Sender<Command>,
    ) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        Self {
            rx,
            tx_cmd,
            ..Default::default()
        }
    }
}

impl eframe::App for LaiApp {
    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // mpsc
        if let Ok(message) = self.rx.try_recv() {
            match message {
                Message::Dates(dates) => {
                    // Set the last date as the selected date
                    self.selected_date = Some(dates.dates.last().unwrap().clone());
                    self.dates = Some(dates);
                },
                Message::Data(data) => {
                    self.modis_data = Some(data);
                }
            }
        }

        // Put your widgets into a `SidePanel`, `TopBottomPanel`, `CentralPanel`, `Window` or `Area`.
        // For inspiration and more examples, go to https://emilk.github.io/egui

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            // The top panel is often a good place for a menu bar:

            egui::menu::bar(ui, |ui| {
                // NOTE: no File->Quit on web pages!
                let is_web = cfg!(target_arch = "wasm32");
                if !is_web {
                    ui.menu_button("File", |ui| {
                        if ui.button("Quit").clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                    ui.add_space(16.0);
                }

                egui::widgets::global_dark_light_mode_buttons(ui);
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            // The central panel the region left after adding TopPanel's and SidePanel's
            ui.heading("Leaf Area Index");

            // If we have dates, display them
            if let Some(dates) = &self.dates {
                // Select box for all calendar dates
                ui.horizontal(|ui| {
                    ui.label("Select a date:");

                    // Combo box
                    egui::ComboBox::from_label("Dates")
                        .selected_text(&self.selected_date.as_ref().unwrap().calendar_date)
                        .show_ui(ui, |ui| {
                            for date in &dates.dates {
                                ui.selectable_value(
                                    self.selected_date.as_mut().unwrap(),
                                    date.clone(),
                                    &date.calendar_date,
                                );
                            }
                        });

                    // Button to get the subset
                    if ui.button("Get Subset").clicked() {
                        let date = &self.selected_date.as_ref().unwrap().modis_date;
                        self.tx_cmd
                            .send(Command::Subset(LAT, LON, date.clone()))
                            .unwrap();
                    }
                });
            } else {
                ui.label("Loading...");
            }

            // If we have data, display it
            // ModisData { xllcorner: "13213215.26", yllcorner: "-5101536.36", cellsize: 463.312716528, nrows: 5, ncols: 5, band: "Lai_500m", units: "m^2/m^2", scale: "0.1", latitude: -45.8667, longitude: 170.6667, header: "https://modisrest.ornl.gov/rst/api/v1/MCD15A2H/subset?latitude=-45.8667&longitude=170.6667&band=Lai_500m&startDate=A2024217&endDate=A2024217&kmAboveBelow=1&kmLeftRight=1", subset: [Subset { modis_date: "A2024217", calendar_date: "2024-08-04", band: "Lai_500m", tile: "h29v13", proc_date: "2024228030354", data: [13, 13, 9, 9, 6, 10, 9, 4, 254, 254, 11, 1, 254, 3, 9, 5, 5, 5, 9, 11, 1, 1, 18, 254, 254] }] }
            // Given nrows, ncols. Plot rectangles of size cellsize and color based on data

            if let Some(data) = &self.modis_data {
                // Draw away from the widgets
                ui.horizontal(|ui| {
                    let next_widget_pos = ui.next_widget_position();

                    // Draw the data
                    for dat in &data.subset {
                        // Get nrows and ncols
                        let nrows = data.nrows;
                        let ncols = data.ncols;

                        // Get cellsize
                        let cellsize = data.cellsize as f32;
                        let cellsize = 25.0;

                        // Get data
                        let data = &dat.data;

                        // Draw the data
                        let mut i = 0;
                        for dat in data {

                            // Determine the x and y offset based off of i
                            let x = i % ncols;
                            let y = i / ncols;

                            // Draw a square
                            let rect = egui::Rect::from_min_size(
                                [x as f32 * cellsize + next_widget_pos.x, y as f32 * cellsize + next_widget_pos.y].into(),
                                [cellsize, cellsize].into(),
                            );

                            // RGB should be a scale from 0 to 255
                            // Transparent if 0 to dark green at 255
                            let color = if *dat == 254 {
                                egui::Color32::TRANSPARENT
                            } else {
                                let scale = (*dat as f32) / 255.0;
                                egui::Color32::from_rgb(
                                    (255.0 * scale) as u8,
                                    (255.0 * (1.0 - scale)) as u8,
                                    0,
                                )
                            };
                            ui.painter().rect_filled(
                                rect,
                                0.0,
                                color,
                            );

                            i += 1;
                        }
                    }

                });
            }
            // Draw a square at 0,0
            // let rect = egui::Rect::from_min_size([0.0, 0.0].into(), [100.0, 100.0].into());
            // ui.painter().rect_filled(rect, 0.0, egui::Color32::from_rgb(100, 200, 100));
        });
    }
}
