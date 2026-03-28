use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};

use crate::TuiApp;

pub fn render(frame: &mut Frame<'_>, area: Rect, app: &TuiApp) {
    let snapshot = &app.snapshot;
    let layout = Layout::vertical([
        Constraint::Length(8),
        Constraint::Length(8),
        Constraint::Min(8),
    ])
    .split(area);
    let top = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(layout[0]);
    let middle = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(layout[1]);

    let plants = Paragraph::new(vec![
        Line::from(format!(
            "Fast stem {:.2} g  health {:.2}",
            snapshot.fast_stem_biomass_g, snapshot.fast_stem_health_index
        )),
        Line::from(format!(
            "Root rosette {:.2} g  health {:.2}",
            snapshot.root_feeding_rosette_biomass_g, snapshot.root_feeding_rosette_health_index
        )),
        Line::from(format!(
            "Substrate N {:.1} mg  P {:.1} mg",
            snapshot.substrate_nutrient_remaining_mg_n_total,
            snapshot.substrate_nutrient_remaining_mg_p_total
        )),
    ])
    .block(Block::default().title("Plant guilds").borders(Borders::ALL));
    frame.render_widget(plants, top[0]);

    let algae = Paragraph::new(vec![
        Line::from(format!(
            "Suspended algae {:.2} g",
            snapshot.suspended_algae_biomass_g
        )),
        Line::from(format!("Periphyton {:.2} g", snapshot.periphyton_biomass_g)),
        Line::from(format!(
            "Nuisance index {:.2}",
            snapshot.algae_nuisance_index
        )),
        Line::from(format!(
            "Detritus particulate {:.2} g  fine {:.2} g",
            snapshot.detritus_particulate_g_total, snapshot.detritus_fine_g_total
        )),
    ])
    .block(
        Block::default()
            .title("Algae and detritus")
            .borders(Borders::ALL),
    );
    frame.render_widget(algae, top[1]);

    let shrimp = Paragraph::new(vec![
        Line::from(format!(
            "Total {}  Adults {}  Sub-adults {}",
            snapshot.total_shrimp_count, snapshot.adult_shrimp_count, snapshot.sub_adult_count
        )),
        Line::from(format!(
            "Juveniles {}  Berried {}",
            snapshot.juveniles_count, snapshot.berried_females_count
        )),
        Line::from(format!(
            snapshot.adult_shrimp_count,
            "Condition {:.2}  Molt stress {:.2}",
            snapshot.shrimp_condition_index, snapshot.shrimp_molt_stress_index
        )),
        Line::from(format!(
            "Readiness {:.2}",
            snapshot.shrimp_reproductive_readiness
        )),
    ])
    .block(
        Block::default()
            .title("Shrimp population")
            .borders(Borders::ALL),
    );
    frame.render_widget(shrimp, middle[0]);

    let biofilter = Paragraph::new(vec![
        Line::from(format!("Maturity {:.2}", snapshot.biofilter_maturity_index)),
        Line::from(format!(
            "AOB {:.4} g  NOB {:.4} g",
            snapshot.ammonia_oxidizer_biomass_g, snapshot.nitrite_oxidizer_biomass_g
        )),
        Line::from(format!(
            "Comammox {:.4} g  Decomposer {:.4} g",
            snapshot.comammox_biomass_g, snapshot.decomposer_biomass_g
        )),
    ])
    .block(Block::default().title("Biofilter").borders(Borders::ALL));
    frame.render_widget(biofilter, middle[1]);

    let microfauna = Paragraph::new(vec![
        Line::from(format!(
            "Population index {:.2}",
            snapshot.microfauna_population_index
        )),
        Line::from(format!(
            "Grazing pressure {:.2}",
            snapshot.microfauna_grazing_pressure_index
        )),
        Line::from(format!(
            "Filter cleanliness {:.2}",
            snapshot.filter_cleanliness_index
        )),
        Line::from(format!(
            "Aeration {} @ {:.2}",
            if snapshot.aeration_enabled {
                "on"
            } else {
                "off"
            },
            snapshot.aeration_intensity
        )),
    ])
    .block(
        Block::default()
            .title("Microfauna summary")
            .borders(Borders::ALL),
    );
    frame.render_widget(microfauna, layout[2]);
}
