use serde::{Deserialize, Serialize};

use super::{SimEvent, TankState};
use crate::systems::temperature::do_sat_mg_l;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TankSnapshot {
    pub day: u32,
    pub hour: u8,
    pub water_temp_c: f64,
    pub water_volume_l: f64,
    pub tan_mg_l: f64,
    pub nh3_mg_l: f64,
    pub nitrite_mg_l: f64,
    pub nitrate_mg_l: f64,
    pub phosphate_mg_l: f64,
    pub do_mg_l: f64,
    pub do_sat_mg_l: f64,
    pub gh_d: f64,
    pub kh_d: f64,
    pub tds_mg_l: f64,
    pub conductivity_us_cm: f64,
    pub ph: f64,
    pub adult_shrimp_count: u32,
    pub total_plant_biomass_g: f64,
    pub last_heater_output_w: f64,
    pub recent_events: Vec<SimEvent>,
}

impl TankSnapshot {
    pub fn from_state(state: &TankState) -> Self {
        let volume_l = state.geometry.water_volume_l();
        let tan_mg_l = safe_div(state.water.ammonia_total_mg_n_total, volume_l);
        let nitrite_mg_l = safe_div(state.water.nitrite_mg_n_total, volume_l);
        let nitrate_mg_l = safe_div(state.water.nitrate_mg_n_total, volume_l);
        let phosphate_mg_l = safe_div(state.water.phosphate_mg_p_total, volume_l);
        let do_mg_l_val = safe_div(state.water.dissolved_oxygen_mg_total, volume_l);
        let ca_mg_l = safe_div(state.water.calcium_mg_total, volume_l);
        let mg_mg_l = safe_div(state.water.magnesium_mg_total, volume_l);
        let alkalinity_meq_l = safe_div(state.water.alkalinity_meq_total, volume_l);
        let total_tracked_ions_mg = state.water.total_tracked_ions_mg();
        let tds_mg_l = safe_div(total_tracked_ions_mg, volume_l);
        let conductivity_us_cm = tds_mg_l / 0.65;
        let gh_d = ((2.497 * ca_mg_l) + (4.118 * mg_mg_l)) / 17.848;
        let kh_d = (alkalinity_meq_l * 50.0) / 17.848;
        let dic_mmol_l = safe_div(
            state.water.dissolved_inorganic_carbon_mg_c_total / 12.0,
            volume_l,
        );
        let ph = (6.3 + safe_log10(alkalinity_meq_l.max(0.05)) - safe_log10(dic_mmol_l.max(0.02)))
            .clamp(5.5, 8.5);
        let pka = 0.09018 + 2729.92 / (273.2 + state.water.temperature_c);
        let fraction_nh3 = 1.0 / (1.0 + 10.0_f64.powf(pka - ph));
        let nh3_mg_l = tan_mg_l * fraction_nh3;

        Self {
            day: state.environment.day,
            hour: state.environment.hour_of_day,
            water_temp_c: state.water.temperature_c,
            water_volume_l: volume_l,
            tan_mg_l,
            nh3_mg_l,
            nitrite_mg_l,
            nitrate_mg_l,
            phosphate_mg_l,
            do_mg_l: do_mg_l_val,
            do_sat_mg_l: do_sat_mg_l(state.water.temperature_c),
            gh_d,
            kh_d,
            tds_mg_l,
            conductivity_us_cm,
            ph,
            adult_shrimp_count: state.animal.adults_count,
            total_plant_biomass_g: state.plant_guilds.iter().map(|plant| plant.biomass_g).sum(),
            last_heater_output_w: state.hardware.heater.last_output_w,
            recent_events: state.event_log.iter().rev().take(20).cloned().collect(),
        }
    }
}

fn safe_div(numerator: f64, denominator: f64) -> f64 {
    if denominator <= f64::EPSILON {
        0.0
    } else {
        numerator / denominator
    }
}

fn safe_log10(value: f64) -> f64 {
    value.max(f64::MIN_POSITIVE).log10()
}
