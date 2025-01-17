use dioxus::prelude::*;
use dioxus_html::FormData;
use dioxus_tui::prelude::*;
use dioxus_tui::Config;

fn main() {
    dioxus_tui::launch_cfg(app, Config::new());
}

fn app(cx: Scope) -> Element {
    let bg_green = use_state(cx, || false);

    let color = if *bg_green.get() { "green" } else { "red" };
    cx.render(rsx! {
        div{
            width: "100%",
            background_color: "{color}",
            flex_direction: "column",
            align_items: "center",
            justify_content: "center",

            Input{
                oninput: |data: FormData| if &data.value == "good"{
                    bg_green.set(true);
                } else{
                    bg_green.set(false);
                },
                r#type: "checkbox",
                value: "good",
                width: "50%",
                height: "10%",
                checked: "true",
            }
            Input{
                oninput: |data: FormData| if &data.value == "hello world"{
                    bg_green.set(true);
                } else{
                    bg_green.set(false);
                },
                width: "50%",
                height: "10%",
                maxlength: "11",
            }
            Input{
                oninput: |data: FormData| {
                    if (data.value.parse::<f32>().unwrap() - 40.0).abs() < 5.0 {
                        bg_green.set(true);
                    } else{
                        bg_green.set(false);
                    }
                },
                r#type: "range",
                width: "50%",
                height: "10%",
                min: "20",
                max: "80",
            }
            Input{
                oninput: |data: FormData| {
                    if data.value == "10"{
                        bg_green.set(true);
                    } else{
                        bg_green.set(false);
                    }
                },
                r#type: "number",
                width: "50%",
                height: "10%",
                maxlength: "4",
            }
            Input{
                oninput: |data: FormData| {
                    if data.value == "hello world"{
                        bg_green.set(true);
                    } else{
                        bg_green.set(false);
                    }
                },
                r#type: "password",
                width: "50%",
                height: "10%",
                maxlength: "11",
            }
            Input{
                onclick: |_: FormData| bg_green.set(true),
                r#type: "button",
                value: "green",
                width: "50%",
                height: "10%",
            }
        }
    })
}
