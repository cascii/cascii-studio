use js_sys::Math;
use once_cell::sync::Lazy;
use serde::Deserialize;
use yew::prelude::*;

const BASE_LINE_HEIGHT_PX: usize = 14;
const HEADERS_JSON: &str = include_str!("../../public/headers.json");

#[derive(Clone, PartialEq, Deserialize)]
struct HeaderVariant {
    id: usize,
    name: String,
    text: String,
    rows: usize,
    cols: usize,
}

static HEADER_VARIANTS: Lazy<Vec<HeaderVariant>> = Lazy::new(|| {
    serde_json::from_str::<Vec<HeaderVariant>>(HEADERS_JSON)
        .unwrap_or_default()
        .into_iter()
        .filter_map(sanitize_variant)
        .collect()
});

fn fallback_variant() -> HeaderVariant {
    let text = "cascii studio".to_string();
    let cols = text.chars().count();
    HeaderVariant {id: 0, name: "fallback".to_string(), text, rows: 1, cols}
}

fn count_visible_chars(line: &str) -> usize {
    line.chars().filter(|ch| *ch != ' ').count()
}

fn sanitize_variant(mut variant: HeaderVariant) -> Option<HeaderVariant> {
    let mut lines: Vec<String> = variant.text.lines().map(str::to_string).collect();

    if lines.len() <= 1 {
        return None;
    }

    while lines.len() > 1 {
        let last_visible = lines.last().map(|line| count_visible_chars(line)).unwrap_or(0);
        let prev_visible = lines.get(lines.len().saturating_sub(2)).map(|line| count_visible_chars(line)).unwrap_or(0);

        if last_visible <= 2 && prev_visible >= 8 {
            lines.pop();
        } else {
            break;
        }
    }

    if lines.is_empty() {
        return None;
    }

    variant.text = lines.join("\n");
    variant.rows = lines.len();
    variant.cols = lines.iter().map(|line| line.chars().count()).max().unwrap_or(1);

    Some(variant)
}

fn select_variant(index: usize) -> HeaderVariant {
    if HEADER_VARIANTS.is_empty() {
        fallback_variant()
    } else {
        HEADER_VARIANTS[index % HEADER_VARIANTS.len()].clone()
    }
}

fn random_variant_index(current: Option<usize>) -> usize {
    let len = HEADER_VARIANTS.len();
    if len <= 1 {
        return 0;
    }

    if let Some(current) = current {
        let raw = (Math::random() * (len - 1) as f64).floor() as usize;
        if raw >= current { raw + 1 } else { raw }
    } else {
        (Math::random() * len as f64).floor() as usize
    }
}

fn art_style(variant: &HeaderVariant) -> AttrValue {
    let art_height_px = variant.rows.max(1) * BASE_LINE_HEIGHT_PX;
    format!("--art-cols:{};--art-height-px:{}px;", variant.cols.max(1), art_height_px).into()
}

#[function_component(Header)]
pub fn header() -> Html {
    let variant_index = use_state(|| random_variant_index(None));
    let variant = select_variant(*variant_index);

    {
        let variant_name = variant.name.clone();
        let variant_id = variant.id;
        use_effect_with(*variant_index, move |index| {
            web_sys::console::log_1(&format!("[header] index={} id={} name={}", index, variant_id, variant_name).into());
            || ()
        });
    }

    let on_click = {
        let variant_index = variant_index.clone();
        Callback::from(move |_| {
            if !HEADER_VARIANTS.is_empty() {
                variant_index.set(random_variant_index(Some(*variant_index)));
            }
        })
    };

    html! {
        <header id="site-header" aria-label="Cascii Studio header" data-variant-id={variant.id.to_string()} data-variant-name={variant.name.clone()} title="Click to view a random header" onclick={on_click}>
            <div id="site-header-inner">
                <pre id="site-header-art" aria-hidden="true" style={art_style(&variant)}>{variant.text.clone()}</pre>
            </div>
        </header>
    }
}
