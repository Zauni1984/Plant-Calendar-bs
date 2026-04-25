use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogEntry {
    pub id: String,
    pub name_de: String,
    pub name_en: String,
    pub category: String,
    pub lifecycle: String,
    pub default_phase: String,
    pub sow_indoor_months: Vec<u8>,
    pub sow_outdoor_months: Vec<u8>,
    pub harvest_months: Vec<u8>,
    pub sun: String,
    pub water: String,
    pub spacing_cm: u16,
    pub good_neighbors: Vec<String>,
    pub bad_neighbors: Vec<String>,
    pub tips_de: String,
    pub color_suggestion: String,
    pub emoji: String,
    // Safety / toxicity
    pub toxic_cats: bool,
    pub toxic_dogs: bool,
    pub toxic_humans: bool,
    pub allergen: bool,
    pub safety_note: String,
    // Propagation
    pub propagation_methods: Vec<String>,
    pub propagation_note: String,
}

fn build_catalog() -> Vec<CatalogEntry> {
    vec![
        // -- Gemuese --
        CatalogEntry {
            id: "tomato".into(), name_de: "Tomate".into(), name_en: "Tomato".into(), category: "gemuese".into(), lifecycle: "annual".into(),
            default_phase: "seed".into(), sow_indoor_months: vec![2,3,4], sow_outdoor_months: vec![5], harvest_months: vec![7,8,9,10],
            sun: "full".into(), water: "high".into(), spacing_cm: 50,
            good_neighbors: vec!["basil".into(),"carrot".into(),"parsley".into(),"marigold".into(),"celery".into()],
            bad_neighbors: vec!["fennel".into(),"kohlrabi".into(),"potato".into(),"cabbage".into()],
            tips_de: "Ausgeizen nicht vergessen. Regenschutz empfohlen.".into(), color_suggestion: "#e74c3c".into(), emoji: "\u{1F345}".into(),
            toxic_cats: true, toxic_dogs: true, toxic_humans: false, allergen: false, safety_note: "Bl\u{00e4}tter und St\u{00e4}ngel sind f\u{00fc}r Haustiere giftig (Solanin).".into(),
            propagation_methods: vec!["steckling".into(),"samen".into()], propagation_note: "Geiztriebe als Stecklinge in Wasser bewurzeln (7-14 Tage).".into(),
        },
        CatalogEntry {
            id: "pepper".into(), name_de: "Paprika".into(), name_en: "Pepper".into(), category: "gemuese".into(), lifecycle: "annual".into(),
            default_phase: "seed".into(), sow_indoor_months: vec![2,3], sow_outdoor_months: vec![5,6], harvest_months: vec![7,8,9,10],
            sun: "full".into(), water: "medium".into(), spacing_cm: 40,
            good_neighbors: vec!["tomato".into(),"basil".into(),"carrot".into(),"onion".into()],
            bad_neighbors: vec!["fennel".into(),"kohlrabi".into()],
            tips_de: "W\u{00e4}rmeliebend. Erst nach Eisheiligen raus.".into(), color_suggestion: "#f39c12".into(), emoji: "\u{1F336}\u{FE0F}".into(),
            toxic_cats: false, toxic_dogs: false, toxic_humans: false, allergen: false, safety_note: "".into(),
            propagation_methods: vec!["samen".into()], propagation_note: "\u{00dc}ber Samen vermehren.".into(),
        },
        CatalogEntry {
            id: "cucumber".into(), name_de: "Gurke".into(), name_en: "Cucumber".into(), category: "gemuese".into(), lifecycle: "annual".into(),
            default_phase: "seed".into(), sow_indoor_months: vec![4], sow_outdoor_months: vec![5,6], harvest_months: vec![7,8,9],
            sun: "full".into(), water: "high".into(), spacing_cm: 40,
            good_neighbors: vec!["bean".into(),"pea".into(),"dill".into(),"lettuce".into(),"sunflower".into()],
            bad_neighbors: vec!["tomato".into(),"potato".into(),"radish".into()],
            tips_de: "Regelm\u{00e4}\u{00df}ig gie\u{00df}en. Rankhilfe nutzen.".into(), color_suggestion: "#27ae60".into(), emoji: "\u{1F952}".into(),
            toxic_cats: false, toxic_dogs: false, toxic_humans: false, allergen: false, safety_note: "".into(),
            propagation_methods: vec!["samen".into()], propagation_note: "\u{00dc}ber Samen vermehren.".into(),
        },
        CatalogEntry {
            id: "zucchini".into(), name_de: "Zucchini".into(), name_en: "Zucchini".into(), category: "gemuese".into(), lifecycle: "annual".into(),
            default_phase: "seed".into(), sow_indoor_months: vec![4], sow_outdoor_months: vec![5], harvest_months: vec![6,7,8,9],
            sun: "full".into(), water: "high".into(), spacing_cm: 80,
            good_neighbors: vec!["bean".into(),"onion".into(),"corn".into(),"sunflower".into()],
            bad_neighbors: vec!["cucumber".into(),"potato".into()],
            tips_de: "Braucht viel Platz. Fr\u{00fc}chte jung ernten.".into(), color_suggestion: "#2ecc71".into(), emoji: "\u{1F96C}".into(),
            toxic_cats: false, toxic_dogs: false, toxic_humans: false, allergen: false, safety_note: "".into(),
            propagation_methods: vec!["samen".into()], propagation_note: "\u{00dc}ber Samen vermehren.".into(),
        },
        CatalogEntry {
            id: "carrot".into(), name_de: "M\u{00f6}hre".into(), name_en: "Carrot".into(), category: "gemuese".into(), lifecycle: "annual".into(),
            default_phase: "seed".into(), sow_indoor_months: vec![], sow_outdoor_months: vec![3,4,5,6], harvest_months: vec![6,7,8,9,10],
            sun: "full".into(), water: "medium".into(), spacing_cm: 5,
            good_neighbors: vec!["onion".into(),"tomato".into(),"pea".into(),"lettuce".into(),"radish".into(),"leek".into()],
            bad_neighbors: vec!["dill".into(),"parsnip".into()],
            tips_de: "Lockerer Boden wichtig. Nicht verpflanzen.".into(), color_suggestion: "#e67e22".into(), emoji: "\u{1F955}".into(),
            toxic_cats: false, toxic_dogs: false, toxic_humans: false, allergen: false, safety_note: "".into(),
            propagation_methods: vec!["samen".into()], propagation_note: "\u{00dc}ber Samen vermehren.".into(),
        },
        CatalogEntry {
            id: "lettuce".into(), name_de: "Salat".into(), name_en: "Lettuce".into(), category: "gemuese".into(), lifecycle: "annual".into(),
            default_phase: "seed".into(), sow_indoor_months: vec![2,3], sow_outdoor_months: vec![3,4,5,6,7,8], harvest_months: vec![4,5,6,7,8,9,10],
            sun: "partial".into(), water: "medium".into(), spacing_cm: 25,
            good_neighbors: vec!["carrot".into(),"radish".into(),"strawberry".into(),"bean".into(),"cucumber".into()],
            bad_neighbors: vec!["parsley".into(),"celery".into()],
            tips_de: "Staffelaussaat f\u{00fc}r durchgehende Ernte.".into(), color_suggestion: "#a8d948".into(), emoji: "\u{1F96C}".into(),
            toxic_cats: false, toxic_dogs: false, toxic_humans: false, allergen: false, safety_note: "".into(),
            propagation_methods: vec!["samen".into()], propagation_note: "\u{00dc}ber Samen vermehren.".into(),
        },
        CatalogEntry {
            id: "bean".into(), name_de: "Bohne".into(), name_en: "Bean".into(), category: "gemuese".into(), lifecycle: "annual".into(),
            default_phase: "seed".into(), sow_indoor_months: vec![], sow_outdoor_months: vec![5,6,7], harvest_months: vec![7,8,9],
            sun: "full".into(), water: "medium".into(), spacing_cm: 10,
            good_neighbors: vec!["corn".into(),"cucumber".into(),"potato".into(),"strawberry".into(),"celery".into()],
            bad_neighbors: vec!["onion".into(),"garlic".into(),"leek".into(),"fennel".into()],
            tips_de: "Stickstoffsammler. Nicht roh essen!".into(), color_suggestion: "#1abc9c".into(), emoji: "\u{1FAD8}".into(),
            toxic_cats: false, toxic_dogs: false, toxic_humans: true, allergen: false, safety_note: "Rohe Bohnen sind giftig!".into(),
            propagation_methods: vec!["samen".into()], propagation_note: "\u{00dc}ber Samen vermehren.".into(),
        },
        CatalogEntry {
            id: "pea".into(), name_de: "Erbse".into(), name_en: "Pea".into(), category: "gemuese".into(), lifecycle: "annual".into(),
            default_phase: "seed".into(), sow_indoor_months: vec![], sow_outdoor_months: vec![3,4,5], harvest_months: vec![6,7,8],
            sun: "full".into(), water: "medium".into(), spacing_cm: 5,
            good_neighbors: vec!["carrot".into(),"radish".into(),"cucumber".into(),"corn".into(),"lettuce".into()],
            bad_neighbors: vec!["onion".into(),"garlic".into(),"leek".into()],
            tips_de: "Kletterger\u{00fc}st aufstellen. K\u{00fc}hle Temperaturen bevorzugt.".into(), color_suggestion: "#2ecc71".into(), emoji: "\u{1FAD9}".into(),
            toxic_cats: false, toxic_dogs: false, toxic_humans: false, allergen: false, safety_note: "".into(),
            propagation_methods: vec!["samen".into()], propagation_note: "\u{00dc}ber Samen vermehren.".into(),
        },
        CatalogEntry {
            id: "potato".into(), name_de: "Kartoffel".into(), name_en: "Potato".into(), category: "gemuese".into(), lifecycle: "annual".into(),
            default_phase: "seed".into(), sow_indoor_months: vec![], sow_outdoor_months: vec![4,5], harvest_months: vec![7,8,9,10],
            sun: "full".into(), water: "medium".into(), spacing_cm: 35,
            good_neighbors: vec!["bean".into(),"corn".into(),"spinach".into(),"cabbage".into()],
            bad_neighbors: vec!["tomato".into(),"cucumber".into(),"sunflower".into()],
            tips_de: "Anh\u{00e4}ufeln f\u{00fc}r mehr Ertrag.".into(), color_suggestion: "#8B6914".into(), emoji: "\u{1F954}".into(),
            toxic_cats: true, toxic_dogs: true, toxic_humans: true, allergen: false, safety_note: "Gr\u{00fc}ne Teile und Keime sind giftig (Solanin).".into(),
            propagation_methods: vec!["samen".into(),"teilung".into()], propagation_note: "Saatkartoffeln vorkeimen und pflanzen.".into(),
        },
        CatalogEntry {
            id: "onion".into(), name_de: "Zwiebel".into(), name_en: "Onion".into(), category: "gemuese".into(), lifecycle: "annual".into(),
            default_phase: "seed".into(), sow_indoor_months: vec![2,3], sow_outdoor_months: vec![3,4], harvest_months: vec![7,8,9],
            sun: "full".into(), water: "low".into(), spacing_cm: 10,
            good_neighbors: vec!["carrot".into(),"strawberry".into(),"lettuce".into(),"tomato".into(),"beet".into()],
            bad_neighbors: vec!["bean".into(),"pea".into(),"cabbage".into()],
            tips_de: "Steckzwiebeln sind einfacher als Samen.".into(), color_suggestion: "#c0811a".into(), emoji: "\u{1F9C5}".into(),
            toxic_cats: true, toxic_dogs: true, toxic_humans: false, allergen: false, safety_note: "Giftig f\u{00fc}r Hunde und Katzen!".into(),
            propagation_methods: vec!["samen".into()], propagation_note: "\u{00dc}ber Samen oder Steckzwiebeln vermehren.".into(),
        },
        CatalogEntry {
            id: "garlic".into(), name_de: "Knoblauch".into(), name_en: "Garlic".into(), category: "gemuese".into(), lifecycle: "annual".into(),
            default_phase: "seed".into(), sow_indoor_months: vec![], sow_outdoor_months: vec![10,3], harvest_months: vec![7,8],
            sun: "full".into(), water: "low".into(), spacing_cm: 15,
            good_neighbors: vec!["strawberry".into(),"tomato".into(),"carrot".into(),"beet".into(),"cucumber".into()],
            bad_neighbors: vec!["bean".into(),"pea".into(),"cabbage".into()],
            tips_de: "Herbstpflanzung f\u{00fc}r gr\u{00f6}\u{00df}ere Knollen.".into(), color_suggestion: "#f5deb3".into(), emoji: "\u{1F9C4}".into(),
            toxic_cats: true, toxic_dogs: true, toxic_humans: false, allergen: false, safety_note: "Giftig f\u{00fc}r Hunde und Katzen!".into(),
            propagation_methods: vec!["teilung".into()], propagation_note: "Einzelne Zehen stecken.".into(),
        },
        CatalogEntry {
            id: "cabbage".into(), name_de: "Kohl".into(), name_en: "Cabbage".into(), category: "gemuese".into(), lifecycle: "annual".into(),
            default_phase: "seed".into(), sow_indoor_months: vec![2,3], sow_outdoor_months: vec![4,5], harvest_months: vec![6,7,8,9,10],
            sun: "full".into(), water: "high".into(), spacing_cm: 50,
            good_neighbors: vec!["bean".into(),"celery".into(),"potato".into(),"beet".into(),"lettuce".into()],
            bad_neighbors: vec!["tomato".into(),"onion".into(),"garlic".into(),"strawberry".into()],
            tips_de: "Kohlfliegennetz verwenden.".into(), color_suggestion: "#7dcea0".into(), emoji: "\u{1F966}".into(),
            toxic_cats: false, toxic_dogs: false, toxic_humans: false, allergen: false, safety_note: "".into(),
            propagation_methods: vec!["samen".into()], propagation_note: "\u{00dc}ber Samen vermehren.".into(),
        },
        CatalogEntry {
            id: "radish".into(), name_de: "Radieschen".into(), name_en: "Radish".into(), category: "gemuese".into(), lifecycle: "annual".into(),
            default_phase: "seed".into(), sow_indoor_months: vec![], sow_outdoor_months: vec![3,4,5,6,7,8,9], harvest_months: vec![4,5,6,7,8,9,10],
            sun: "partial".into(), water: "medium".into(), spacing_cm: 5,
            good_neighbors: vec!["carrot".into(),"pea".into(),"lettuce".into(),"bean".into(),"spinach".into()],
            bad_neighbors: vec!["cucumber".into()],
            tips_de: "Schnellwachser. Ideal als L\u{00fc}ckenf\u{00fc}ller.".into(), color_suggestion: "#e74c6f".into(), emoji: "\u{1F534}".into(),
            toxic_cats: false, toxic_dogs: false, toxic_humans: false, allergen: false, safety_note: "".into(),
            propagation_methods: vec!["samen".into()], propagation_note: "\u{00dc}ber Samen vermehren.".into(),
        },
        CatalogEntry {
            id: "spinach".into(), name_de: "Spinat".into(), name_en: "Spinach".into(), category: "gemuese".into(), lifecycle: "annual".into(),
            default_phase: "seed".into(), sow_indoor_months: vec![], sow_outdoor_months: vec![3,4,5,8,9], harvest_months: vec![4,5,6,9,10,11],
            sun: "partial".into(), water: "medium".into(), spacing_cm: 15,
            good_neighbors: vec!["strawberry".into(),"bean".into(),"potato".into(),"radish".into(),"cabbage".into()],
            bad_neighbors: vec!["beet".into()],
            tips_de: "Schosst schnell bei Hitze. Herbstaussaat lohnt.".into(), color_suggestion: "#196F3D".into(), emoji: "\u{1F96C}".into(),
            toxic_cats: false, toxic_dogs: false, toxic_humans: false, allergen: false, safety_note: "".into(),
            propagation_methods: vec!["samen".into()], propagation_note: "\u{00dc}ber Samen vermehren.".into(),
        },
        CatalogEntry {
            id: "corn".into(), name_de: "Mais".into(), name_en: "Corn".into(), category: "gemuese".into(), lifecycle: "annual".into(),
            default_phase: "seed".into(), sow_indoor_months: vec![4], sow_outdoor_months: vec![5], harvest_months: vec![8,9,10],
            sun: "full".into(), water: "high".into(), spacing_cm: 30,
            good_neighbors: vec!["bean".into(),"cucumber".into(),"pea".into(),"potato".into(),"zucchini".into()],
            bad_neighbors: vec!["celery".into(),"beet".into()],
            tips_de: "In Bl\u{00f6}cken s\u{00e4}en f\u{00fc}r bessere Best\u{00e4}ubung.".into(), color_suggestion: "#f4d03f".into(), emoji: "\u{1F33D}".into(),
            toxic_cats: false, toxic_dogs: false, toxic_humans: false, allergen: false, safety_note: "".into(),
            propagation_methods: vec!["samen".into()], propagation_note: "\u{00dc}ber Samen vermehren.".into(),
        },
        CatalogEntry {
            id: "beet".into(), name_de: "Rote Bete".into(), name_en: "Beetroot".into(), category: "gemuese".into(), lifecycle: "annual".into(),
            default_phase: "seed".into(), sow_indoor_months: vec![], sow_outdoor_months: vec![4,5,6], harvest_months: vec![7,8,9,10],
            sun: "full".into(), water: "medium".into(), spacing_cm: 10,
            good_neighbors: vec!["onion".into(),"garlic".into(),"lettuce".into(),"cabbage".into(),"bean".into()],
            bad_neighbors: vec!["spinach".into(),"corn".into()],
            tips_de: "Jedes Samenkn\u{00e4}uel ergibt mehrere Pflanzen.".into(), color_suggestion: "#900020".into(), emoji: "\u{1F7E3}".into(),
            toxic_cats: false, toxic_dogs: false, toxic_humans: false, allergen: false, safety_note: "".into(),
            propagation_methods: vec!["samen".into()], propagation_note: "\u{00dc}ber Samen vermehren.".into(),
        },
        CatalogEntry {
            id: "leek".into(), name_de: "Lauch".into(), name_en: "Leek".into(), category: "gemuese".into(), lifecycle: "annual".into(),
            default_phase: "seed".into(), sow_indoor_months: vec![2,3], sow_outdoor_months: vec![4,5], harvest_months: vec![9,10,11,12],
            sun: "full".into(), water: "medium".into(), spacing_cm: 15,
            good_neighbors: vec!["carrot".into(),"celery".into(),"strawberry".into(),"tomato".into()],
            bad_neighbors: vec!["bean".into(),"pea".into()],
            tips_de: "Tief pflanzen und anh\u{00e4}ufeln f\u{00fc}r wei\u{00df}en Schaft.".into(), color_suggestion: "#82E0AA".into(), emoji: "\u{1F9C5}".into(),
            toxic_cats: true, toxic_dogs: true, toxic_humans: false, allergen: false, safety_note: "Lauchgew\u{00e4}chse giftig f\u{00fc}r Haustiere.".into(),
            propagation_methods: vec!["samen".into()], propagation_note: "\u{00dc}ber Samen vermehren.".into(),
        },
        CatalogEntry {
            id: "celery".into(), name_de: "Sellerie".into(), name_en: "Celery".into(), category: "gemuese".into(), lifecycle: "annual".into(),
            default_phase: "seed".into(), sow_indoor_months: vec![2,3], sow_outdoor_months: vec![], harvest_months: vec![9,10,11],
            sun: "full".into(), water: "high".into(), spacing_cm: 35,
            good_neighbors: vec!["tomato".into(),"cabbage".into(),"leek".into(),"bean".into()],
            bad_neighbors: vec!["lettuce".into(),"corn".into(),"potato".into()],
            tips_de: "Lichtkeimer. Lange Vorkultur n\u{00f6}tig.".into(), color_suggestion: "#abebc6".into(), emoji: "\u{1F96C}".into(),
            toxic_cats: false, toxic_dogs: false, toxic_humans: false, allergen: true, safety_note: "Kann Kontaktallergien ausl\u{00f6}sen.".into(),
            propagation_methods: vec!["samen".into()], propagation_note: "\u{00dc}ber Samen vermehren.".into(),
        },
        // -- Kraeuter --
        CatalogEntry {
            id: "basil".into(), name_de: "Basilikum".into(), name_en: "Basil".into(), category: "kraeuter".into(), lifecycle: "annual".into(),
            default_phase: "seed".into(), sow_indoor_months: vec![3,4], sow_outdoor_months: vec![5,6], harvest_months: vec![6,7,8,9],
            sun: "full".into(), water: "medium".into(), spacing_cm: 20,
            good_neighbors: vec!["tomato".into(),"pepper".into(),"cucumber".into()],
            bad_neighbors: vec!["sage".into(),"rue".into()],
            tips_de: "Kein Frost! Regelm\u{00e4}\u{00df}ig ernten f\u{00f6}rdert Wachstum.".into(), color_suggestion: "#27ae60".into(), emoji: "\u{1F33F}".into(),
            toxic_cats: false, toxic_dogs: false, toxic_humans: false, allergen: false, safety_note: "".into(),
            propagation_methods: vec!["steckling".into(),"samen".into()], propagation_note: "Triebspitzen in Wasser stellen, nach 1 Woche Wurzeln.".into(),
        },
        CatalogEntry {
            id: "parsley".into(), name_de: "Petersilie".into(), name_en: "Parsley".into(), category: "kraeuter".into(), lifecycle: "annual".into(),
            default_phase: "seed".into(), sow_indoor_months: vec![2,3], sow_outdoor_months: vec![4,5], harvest_months: vec![5,6,7,8,9,10],
            sun: "partial".into(), water: "medium".into(), spacing_cm: 15,
            good_neighbors: vec!["tomato".into(),"carrot".into(),"onion".into(),"radish".into()],
            bad_neighbors: vec!["lettuce".into(),"celery".into()],
            tips_de: "Keimt langsam (2-4 Wochen). Geduld!".into(), color_suggestion: "#229954".into(), emoji: "\u{1F33F}".into(),
            toxic_cats: true, toxic_dogs: false, toxic_humans: false, allergen: false, safety_note: "In gro\u{00df}en Mengen problematisch f\u{00fc}r Katzen.".into(),
            propagation_methods: vec!["samen".into()], propagation_note: "\u{00dc}ber Samen vermehren.".into(),
        },
        CatalogEntry {
            id: "dill".into(), name_de: "Dill".into(), name_en: "Dill".into(), category: "kraeuter".into(), lifecycle: "annual".into(),
            default_phase: "seed".into(), sow_indoor_months: vec![], sow_outdoor_months: vec![4,5,6,7], harvest_months: vec![5,6,7,8,9],
            sun: "full".into(), water: "medium".into(), spacing_cm: 20,
            good_neighbors: vec!["cucumber".into(),"lettuce".into(),"onion".into(),"cabbage".into()],
            bad_neighbors: vec!["carrot".into(),"fennel".into()],
            tips_de: "S\u{00e4}t sich gerne selbst aus.".into(), color_suggestion: "#52BE80".into(), emoji: "\u{1F33F}".into(),
            toxic_cats: false, toxic_dogs: false, toxic_humans: false, allergen: false, safety_note: "".into(),
            propagation_methods: vec!["samen".into()], propagation_note: "S\u{00e4}t sich gerne selbst aus.".into(),
        },
        CatalogEntry {
            id: "mint".into(), name_de: "Minze".into(), name_en: "Mint".into(), category: "kraeuter".into(), lifecycle: "perennial".into(),
            default_phase: "veg".into(), sow_indoor_months: vec![3,4], sow_outdoor_months: vec![5], harvest_months: vec![5,6,7,8,9],
            sun: "partial".into(), water: "high".into(), spacing_cm: 30,
            good_neighbors: vec!["tomato".into(),"cabbage".into()],
            bad_neighbors: vec!["parsley".into()],
            tips_de: "Wuchert stark! Am besten im Topf halten.".into(), color_suggestion: "#1abc9c".into(), emoji: "\u{1F33F}".into(),
            toxic_cats: true, toxic_dogs: false, toxic_humans: false, allergen: false, safety_note: "\u{00c4}therische \u{00d6}le problematisch f\u{00fc}r Katzen.".into(),
            propagation_methods: vec!["ableger".into(),"teilung".into(),"steckling".into()], propagation_note: "Ausl\u{00e4}ufer teilen oder Stecklinge in Wasser.".into(),
        },
        CatalogEntry {
            id: "rosemary".into(), name_de: "Rosmarin".into(), name_en: "Rosemary".into(), category: "kraeuter".into(), lifecycle: "perennial".into(),
            default_phase: "veg".into(), sow_indoor_months: vec![3,4], sow_outdoor_months: vec![5], harvest_months: vec![1,2,3,4,5,6,7,8,9,10,11,12],
            sun: "full".into(), water: "low".into(), spacing_cm: 40,
            good_neighbors: vec!["bean".into(),"cabbage".into(),"carrot".into()],
            bad_neighbors: vec!["basil".into()],
            tips_de: "Winterschutz in kalten Regionen. Wenig gie\u{00df}en.".into(), color_suggestion: "#5B8C5A".into(), emoji: "\u{1F33F}".into(),
            toxic_cats: true, toxic_dogs: false, toxic_humans: false, allergen: false, safety_note: "In gro\u{00df}en Mengen problematisch f\u{00fc}r Katzen.".into(),
            propagation_methods: vec!["steckling".into()], propagation_note: "Verholzte Triebe im Fr\u{00fc}hjahr schneiden, 4-6 Wochen bewurzeln.".into(),
        },
        CatalogEntry {
            id: "thyme".into(), name_de: "Thymian".into(), name_en: "Thyme".into(), category: "kraeuter".into(), lifecycle: "perennial".into(),
            default_phase: "veg".into(), sow_indoor_months: vec![3,4], sow_outdoor_months: vec![5], harvest_months: vec![4,5,6,7,8,9,10],
            sun: "full".into(), water: "low".into(), spacing_cm: 25,
            good_neighbors: vec!["cabbage".into(),"strawberry".into(),"tomato".into()],
            bad_neighbors: vec![],
            tips_de: "Trockenheitsvertr\u{00e4}glich. R\u{00fc}ckschnitt im Fr\u{00fc}hjahr.".into(), color_suggestion: "#7DCEA0".into(), emoji: "\u{1F33F}".into(),
            toxic_cats: false, toxic_dogs: false, toxic_humans: false, allergen: false, safety_note: "".into(),
            propagation_methods: vec!["steckling".into(),"teilung".into(),"samen".into()], propagation_note: "Stecklinge oder Teilung im Fr\u{00fc}hjahr.".into(),
        },
        CatalogEntry {
            id: "chives".into(), name_de: "Schnittlauch".into(), name_en: "Chives".into(), category: "kraeuter".into(), lifecycle: "perennial".into(),
            default_phase: "veg".into(), sow_indoor_months: vec![2,3], sow_outdoor_months: vec![4,5], harvest_months: vec![3,4,5,6,7,8,9,10],
            sun: "partial".into(), water: "medium".into(), spacing_cm: 15,
            good_neighbors: vec!["carrot".into(),"tomato".into(),"strawberry".into()],
            bad_neighbors: vec!["bean".into(),"pea".into()],
            tips_de: "Regelm\u{00e4}\u{00df}ig schneiden. Bl\u{00fc}ten sind essbar.".into(), color_suggestion: "#AF7AC5".into(), emoji: "\u{1F33F}".into(),
            toxic_cats: true, toxic_dogs: true, toxic_humans: false, allergen: false, safety_note: "Lauchgew\u{00e4}chse giftig f\u{00fc}r Haustiere.".into(),
            propagation_methods: vec!["teilung".into(),"samen".into()], propagation_note: "Horste im Fr\u{00fc}hjahr oder Herbst teilen.".into(),
        },
        // -- Obst / Beeren --
        CatalogEntry {
            id: "strawberry".into(), name_de: "Erdbeere".into(), name_en: "Strawberry".into(), category: "beeren".into(), lifecycle: "perennial".into(),
            default_phase: "veg".into(), sow_indoor_months: vec![], sow_outdoor_months: vec![7,8], harvest_months: vec![5,6,7],
            sun: "full".into(), water: "medium".into(), spacing_cm: 30,
            good_neighbors: vec!["lettuce".into(),"spinach".into(),"onion".into(),"garlic".into(),"bean".into(),"thyme".into(),"chives".into()],
            bad_neighbors: vec!["cabbage".into()],
            tips_de: "Mulchen gegen Grauschimmel. Ausl\u{00e4}ufer f\u{00fc}r Vermehrung.".into(), color_suggestion: "#e74c3c".into(), emoji: "\u{1F353}".into(),
            toxic_cats: false, toxic_dogs: false, toxic_humans: false, allergen: true, safety_note: "Kann Allergien ausl\u{00f6}sen.".into(),
            propagation_methods: vec!["ableger".into()], propagation_note: "Ausl\u{00e4}ufer im Sommer direkt in T\u{00f6}pfe leiten.".into(),
        },
        CatalogEntry {
            id: "raspberry".into(), name_de: "Himbeere".into(), name_en: "Raspberry".into(), category: "beeren".into(), lifecycle: "perennial".into(),
            default_phase: "veg".into(), sow_indoor_months: vec![], sow_outdoor_months: vec![10,11,3], harvest_months: vec![6,7,8,9],
            sun: "full".into(), water: "medium".into(), spacing_cm: 50,
            good_neighbors: vec!["garlic".into(),"onion".into()],
            bad_neighbors: vec!["strawberry".into(),"potato".into()],
            tips_de: "Sommerhimbeeren: alte Ruten nach Ernte schneiden.".into(), color_suggestion: "#C0392B".into(), emoji: "\u{1FAD0}".into(),
            toxic_cats: false, toxic_dogs: false, toxic_humans: false, allergen: false, safety_note: "".into(),
            propagation_methods: vec!["ableger".into(),"teilung".into()], propagation_note: "Wurzelausl\u{00e4}ufer im Herbst trennen.".into(),
        },
        CatalogEntry {
            id: "blueberry".into(), name_de: "Heidelbeere".into(), name_en: "Blueberry".into(), category: "beeren".into(), lifecycle: "perennial".into(),
            default_phase: "veg".into(), sow_indoor_months: vec![], sow_outdoor_months: vec![3,4,10], harvest_months: vec![7,8,9],
            sun: "full".into(), water: "medium".into(), spacing_cm: 100,
            good_neighbors: vec!["strawberry".into(),"cranberry".into()],
            bad_neighbors: vec![],
            tips_de: "Braucht sauren Boden (pH 4-5). Rhododendronerde verwenden.".into(), color_suggestion: "#2E4053".into(), emoji: "\u{1FAD0}".into(),
            toxic_cats: false, toxic_dogs: false, toxic_humans: false, allergen: false, safety_note: "".into(),
            propagation_methods: vec!["steckling".into()], propagation_note: "Stecklinge im Sommer schneiden.".into(),
        },
        CatalogEntry {
            id: "apple".into(), name_de: "Apfelbaum".into(), name_en: "Apple Tree".into(), category: "obst".into(), lifecycle: "perennial".into(),
            default_phase: "veg".into(), sow_indoor_months: vec![], sow_outdoor_months: vec![10,11,3], harvest_months: vec![8,9,10],
            sun: "full".into(), water: "medium".into(), spacing_cm: 400,
            good_neighbors: vec!["chives".into(),"nasturtium".into(),"marigold".into()],
            bad_neighbors: vec!["walnut".into()],
            tips_de: "Winterschnitt Januar-M\u{00e4}rz. Braucht Befruchtersorte.".into(), color_suggestion: "#7FB800".into(), emoji: "\u{1F34E}".into(),
            toxic_cats: true, toxic_dogs: true, toxic_humans: true, allergen: false, safety_note: "Kerne enthalten Blaus\u{00e4}ure.".into(),
            propagation_methods: vec!["veredelung".into()], propagation_note: "Veredelung auf Unterlage, nicht \u{00fc}ber Samen sortenecht.".into(),
        },
        CatalogEntry {
            id: "grape".into(), name_de: "Weinrebe".into(), name_en: "Grape Vine".into(), category: "obst".into(), lifecycle: "perennial".into(),
            default_phase: "veg".into(), sow_indoor_months: vec![], sow_outdoor_months: vec![3,4], harvest_months: vec![9,10],
            sun: "full".into(), water: "low".into(), spacing_cm: 200,
            good_neighbors: vec!["rosemary".into(),"basil".into(),"marigold".into()],
            bad_neighbors: vec!["cabbage".into()],
            tips_de: "Winterschnitt wichtig. Regengesch\u{00fc}tzt pflanzen.".into(), color_suggestion: "#6C3483".into(), emoji: "\u{1F347}".into(),
            toxic_cats: true, toxic_dogs: true, toxic_humans: false, allergen: false, safety_note: "Trauben sind giftig f\u{00fc}r Hunde und Katzen!".into(),
            propagation_methods: vec!["steckling".into()], propagation_note: "Winterstecklinge im Februar schneiden.".into(),
        },
        // -- Blumen --
        CatalogEntry {
            id: "sunflower".into(), name_de: "Sonnenblume".into(), name_en: "Sunflower".into(), category: "blumen".into(), lifecycle: "annual".into(),
            default_phase: "seed".into(), sow_indoor_months: vec![4], sow_outdoor_months: vec![5], harvest_months: vec![8,9,10],
            sun: "full".into(), water: "medium".into(), spacing_cm: 50,
            good_neighbors: vec!["cucumber".into(),"corn".into(),"zucchini".into()],
            bad_neighbors: vec!["potato".into()],
            tips_de: "Bienenweide! Kerne im Herbst ernten.".into(), color_suggestion: "#F4D03F".into(), emoji: "\u{1F33B}".into(),
            toxic_cats: false, toxic_dogs: false, toxic_humans: false, allergen: false, safety_note: "".into(),
            propagation_methods: vec!["samen".into()], propagation_note: "\u{00dc}ber Samen vermehren.".into(),
        },
        CatalogEntry {
            id: "marigold".into(), name_de: "Tagetes".into(), name_en: "Marigold".into(), category: "blumen".into(), lifecycle: "annual".into(),
            default_phase: "seed".into(), sow_indoor_months: vec![3,4], sow_outdoor_months: vec![5], harvest_months: vec![6,7,8,9,10],
            sun: "full".into(), water: "medium".into(), spacing_cm: 20,
            good_neighbors: vec!["tomato".into(),"pepper".into(),"cabbage".into(),"bean".into(),"apple".into()],
            bad_neighbors: vec![],
            tips_de: "Vertreibt Nematoden. Ideal als Beetumrandung.".into(), color_suggestion: "#F5B041".into(), emoji: "\u{1F33C}".into(),
            toxic_cats: true, toxic_dogs: true, toxic_humans: false, allergen: true, safety_note: "Leicht giftig f\u{00fc}r Haustiere. Kontaktallergie m\u{00f6}glich.".into(),
            propagation_methods: vec!["samen".into()], propagation_note: "\u{00dc}ber Samen vermehren. S\u{00e4}t sich oft selbst aus.".into(),
        },
        CatalogEntry {
            id: "nasturtium".into(), name_de: "Kapuzinerkresse".into(), name_en: "Nasturtium".into(), category: "blumen".into(), lifecycle: "annual".into(),
            default_phase: "seed".into(), sow_indoor_months: vec![4], sow_outdoor_months: vec![5], harvest_months: vec![6,7,8,9],
            sun: "full".into(), water: "low".into(), spacing_cm: 30,
            good_neighbors: vec!["apple".into(),"cabbage".into(),"cucumber".into(),"tomato".into()],
            bad_neighbors: vec![],
            tips_de: "Bl\u{00fc}ten und Bl\u{00e4}tter sind essbar. Lockt Blattl\u{00e4}use an (Opferpflanze).".into(), color_suggestion: "#E67E22".into(), emoji: "\u{1F33A}".into(),
            toxic_cats: false, toxic_dogs: false, toxic_humans: false, allergen: false, safety_note: "Essbar und ungiftig.".into(),
            propagation_methods: vec!["samen".into()], propagation_note: "\u{00dc}ber Samen vermehren.".into(),
        },
        // -- Cannabis (CSC) --
        CatalogEntry {
            id: "cannabis".into(), name_de: "Cannabis".into(), name_en: "Cannabis".into(), category: "gemuese".into(), lifecycle: "annual".into(),
            default_phase: "seed".into(), sow_indoor_months: vec![3,4,5], sow_outdoor_months: vec![5,6], harvest_months: vec![9,10,11],
            sun: "full".into(), water: "high".into(), spacing_cm: 60,
            good_neighbors: vec!["basil".into(),"marigold".into(),"mint".into(),"dill".into()],
            bad_neighbors: vec![],
            tips_de: "Photoperiodisch: 12/12 f\u{00fc}r Bl\u{00fc}te. pH 6.0-7.0, EC 1.0-2.0.".into(), color_suggestion: "#0B3D2E".into(), emoji: "\u{1F33F}".into(),
            toxic_cats: true, toxic_dogs: true, toxic_humans: false, allergen: true, safety_note: "Giftig f\u{00fc}r Haustiere, allergieausl\u{00f6}send.".into(),
            propagation_methods: vec!["steckling".into(),"samen".into()], propagation_note: "Stecklinge unter 18h Licht bewurzeln (10-14 Tage).".into(),
        },
    ]
}

static CATALOG: OnceLock<Vec<CatalogEntry>> = OnceLock::new();

/// Returns the full plant catalog.
pub fn plant_catalog() -> &'static Vec<CatalogEntry> {
    CATALOG.get_or_init(build_catalog)
}

/// Look up a catalog entry by ID.
pub fn catalog_lookup(id: &str) -> Option<&'static CatalogEntry> {
    plant_catalog().iter().find(|e| e.id == id)
}

/// Get companion planting info between two plants.
pub fn companion_status(plant_a: &str, plant_b: &str) -> CompanionResult {
    if let Some(entry) = catalog_lookup(plant_a) {
        if entry.good_neighbors.iter().any(|n| n == plant_b) {
            return CompanionResult::Good;
        }
        if entry.bad_neighbors.iter().any(|n| n == plant_b) {
            return CompanionResult::Bad;
        }
    }
    if let Some(entry) = catalog_lookup(plant_b) {
        if entry.good_neighbors.iter().any(|n| n == plant_a) {
            return CompanionResult::Good;
        }
        if entry.bad_neighbors.iter().any(|n| n == plant_a) {
            return CompanionResult::Bad;
        }
    }
    CompanionResult::Neutral
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompanionResult {
    Good,
    Bad,
    Neutral,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_has_entries() {
        assert!(plant_catalog().len() > 30);
    }

    #[test]
    fn lookup_tomato() {
        let entry = catalog_lookup("tomato").unwrap();
        assert_eq!(entry.name_de, "Tomate");
        assert_eq!(entry.category, "gemuese");
    }

    #[test]
    fn companion_good() {
        assert_eq!(companion_status("tomato", "basil"), CompanionResult::Good);
    }

    #[test]
    fn companion_bad() {
        assert_eq!(companion_status("tomato", "fennel"), CompanionResult::Bad);
    }

    #[test]
    fn companion_symmetric() {
        // basil lists tomato as good neighbor
        assert_eq!(companion_status("basil", "tomato"), CompanionResult::Good);
    }

    #[test]
    fn companion_neutral() {
        assert_eq!(companion_status("tomato", "blueberry"), CompanionResult::Neutral);
    }

    #[test]
    fn companion_unknown_plant() {
        assert_eq!(companion_status("tomato", "unknown_plant"), CompanionResult::Neutral);
    }
}
