use gloo_timers::callback::Timeout;
use wasm_bindgen::prelude::*;
use yew::prelude::*;
use yew_icons::{Icon, IconId};

#[wasm_bindgen(inline_js = "export function copy_to_clipboard(text) { navigator.clipboard.writeText(text); }")]
extern "C" {
    fn copy_to_clipboard(text: &str);
}

#[function_component(SponsorPage)]
pub fn sponsor_page() -> Html {
    let recently_copied = use_state(|| String::new());

    let make_copy_callback =
        |address: &'static str, id: &'static str, recently_copied: UseStateHandle<String>| {
            Callback::from(move |_: MouseEvent| {
                copy_to_clipboard(address);
                let id_str = id.to_string();
                recently_copied.set(id_str);
                let recently_copied_clone = recently_copied.clone();
                let timeout = Timeout::new(1500, move || {
                    recently_copied_clone.set(String::new());
                });
                timeout.forget();
            })
        };

    let btc_addr = "bc1q99v9y7kt9ayu4r4ftxk2znzdcq5ca9fv988m5q";
    let eth_addr = "0x66994e0929576881B752a2BB8C249c9C8e74C253";

    html! {
        <div class="container" id="sponsor-container">
            <h1>{"Support cascii Development"}</h1>
            <p class="sponsor-subtitle">{"If you find cascii useful, please consider supporting its development."}</p>

            <div class="donation-addresses">
                <div class="address-card">
                    <span class="address-label">{"BTC"}</span>
                    <span class="address-value">{btc_addr}</span>
                    <button
                        class="icon-btn copy-btn" onclick={make_copy_callback(btc_addr, "btc", recently_copied.clone())} title="Copy address">
                        <Icon icon_id={if *recently_copied == "btc" { IconId::LucideCheck } else { IconId::LucideCopy }} width={"16"} height={"16"} />
                    </button>
                </div>
                <div class="address-card">
                    <span class="address-label">{"ETH (ERC20)"}</span>
                    <span class="address-value">{eth_addr}</span>
                    <button class="icon-btn copy-btn" onclick={make_copy_callback(eth_addr, "eth", recently_copied.clone())} title="Copy address">
                        <Icon icon_id={if *recently_copied == "eth" { IconId::LucideCheck } else { IconId::LucideCopy }} width={"16"} height={"16"} />
                    </button>
                </div>
                <div class="address-card">
                    <span class="address-label">{"USDT (ERC20)"}</span>
                    <span class="address-value">{eth_addr}</span>
                    <button class="icon-btn copy-btn" onclick={make_copy_callback(eth_addr, "usdt", recently_copied.clone())} title="Copy address">
                        <Icon icon_id={if *recently_copied == "usdt" { IconId::LucideCheck } else { IconId::LucideCopy }} width={"16"} height={"16"} />
                    </button>
                </div>
            </div>
        </div>
    }
}
