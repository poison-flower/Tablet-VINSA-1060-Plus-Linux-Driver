use eframe::egui;
use crate::config::AppConfig;

pub fn run_gui() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([400.0, 400.0]),
        ..Default::default()
    };
    let app = ConfigEditor::new(AppConfig::load());
    
    eframe::run_native(
        "Tablet Settings",
        options,
        Box::new(|_cc| Ok(Box::new(app))),
    )
}

struct ConfigEditor {
    config: AppConfig,
    status_msg: String,
}

impl ConfigEditor {
    fn new(config: AppConfig) -> Self {
        Self {
            config,
            status_msg: String::new(),
        }
    }
}

impl eframe::App for ConfigEditor {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {      
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label("Pressure Threshold:");
            ui.add(egui::Slider::new(
                &mut self.config.pressure_threshold, 
                0..=2048)
                .text("levels")
                .step_by(10.0)
            );

            ui.add_space(10.0);

            ui.label("Sensitivity:");
            ui.add(egui::Slider::new(
                &mut self.config.sensitivity, 
                0.1..=10.0)
                .text("x")
                .step_by(0.1)
            );

            ui.separator();

            if ui.button("Save").clicked() {
                match self.config.save() {
                    Ok(_) => self.status_msg = "Saved!".to_string(),
                    Err(e) => self.status_msg = format!("Error: {}", e),
                }
            }
            
            if !self.status_msg.is_empty() {
                ui.label(&self.status_msg);
            }
        });
    }
    
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        let _ = self.config.save();
    }
}