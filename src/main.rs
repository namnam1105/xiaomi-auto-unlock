// main.rs
mod deviceid;
mod logger;
mod network;

use chrono::NaiveTime;
use chrono_tz::Asia::Shanghai;
use network::{
    check_unlock_status, get_synchronized_beijing_time, wait_until_ping_time,
    wait_until_target_time,
};
use reqwest::{Client, header::HeaderValue};
use serde_json::Value;
use slint::ComponentHandle;
use std::{
    collections::HashMap,
    error::Error,
};

slint::include_modules!();

use logger::{log, update_status};
use tokio::spawn;

async fn on_submit(session: &Client, cookie_value: String, device_id: String) {
    let cookie_value = cookie_value.clone();
    if check_unlock_status(&session, cookie_value.as_str(), device_id.as_str()).await {
        let start_beijing_time = network::get_initial_beijing_time().await;
        if start_beijing_time.is_none() {
            log("Ошибка получения начального времени".to_string());
            return;
        } else {
            let start_timestamp = start_beijing_time.unwrap().timestamp();
            let avg_ping =
                wait_until_ping_time(start_beijing_time.unwrap(), start_timestamp).await;
            wait_until_target_time(
                start_beijing_time.unwrap(),
                start_timestamp,
                avg_ping as u64,
            )
            .await;
            let url = "https://sgp-api.buy.mi.com/bbs/api/global/apply/bl-auth";
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert("Cookie", HeaderValue::from_str(format!("new_bbs_serviceToken={cookie_value};versionCode=500411;versionName=5.4.11;deviceId={device_id};").as_str()).unwrap());
            headers.insert("User-Agent", HeaderValue::from_str("okhttp/4.9.3").unwrap());
            headers.insert(
                "Accept-Encoding",
                HeaderValue::from_str("gzip, deflate, br").unwrap(),
            );
            headers.insert("Connection", HeaderValue::from_str("keep-alive").unwrap());

            let request_time =
                get_synchronized_beijing_time(start_beijing_time.unwrap(), start_timestamp)
                    .await;
            log(format!(
                "Отправка запроса в {} (Пекинское время)",
                request_time
            ));
            let response = session.post(url).headers(headers).send().await;
            if response.is_err() {
                log(format!(
                    "Ошибка отправки запроса: {}",
                    response.unwrap_err()
                ));
                return;
            } else {
                let response_time =
                    get_synchronized_beijing_time(start_beijing_time.unwrap(), start_timestamp)
                        .await;
                log(format!(
                    "Ответ получен в {} (Пекинское время)",
                    response_time
                ));
                let response_data: HashMap<String, Value> =
                    response.unwrap().json().await.unwrap();
                let code = response_data.get("code").unwrap();
                let data = response_data.get("data").unwrap();
                if code == &Value::Number(0.into()) {
                    let apply_result = data.get("apply_result").unwrap();
                    let apply_result = apply_result.as_i64().unwrap();
                    if apply_result == 1 {
                        log("[Статус] Заявка одобрена, проверяем статус...");
                        check_unlock_status(session, cookie_value.as_str(), device_id.as_str())
                            .await;
                    } else if apply_result == 3 {
                        let deadline_format = data.get("deadline_format");
                        log(format!("[Статус] Заявка не подана, исчерпан лимит (Попробуйте привзять телефон в настройках в стасут Mi Unlock), попробуйте снова в {} (Месяц/День).", if deadline_format.is_none() {"Не указано".to_string()} else { deadline_format.unwrap().as_str().unwrap().to_string() }));
                        return
                    } else if apply_result == 4 {
                        let deadline_format = data.get("deadline_format");
                        log(format!("[Статус] Заявка не подана, выдана блокировка на подачу заявки до {} (Месяц/День).", if deadline_format.is_none() {"Не указано".to_string()} else { deadline_format.unwrap().as_str().unwrap().to_string() }));                            
                        let midnight_beijing = start_beijing_time.unwrap().date_naive().and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap());
                        let midnight_beijing = midnight_beijing.and_utc().with_timezone(&Shanghai);
                        let midnight_beijing = midnight_beijing + chrono::Duration::days(1);
                        let response_time_beijing = response_time.with_timezone(&Shanghai);
                        let time_diff = midnight_beijing - response_time_beijing;
                        let time_diff = time_diff.num_milliseconds() as f64 / 1000 as f64;
                        if time_diff <= 3.35 {
                            log("Ваша заявка была принята, зайдите в настройки телефона для привязки");
                            update_status(true, "Заявка принята");
                        } else {
                            log("Не удача, заявка подана слишком поздно");
                            update_status(false, "Ошибка");
                        }
                    }
                } else if code == &Value::Number(100001.into()) {
                    log("[Статус] Заявка отклонена, ошибка запроса (code 100001).")
                } else if code == &Value::Number(100003.into()) {
                    log("[Статус] Возможно заявка одобрена, проверяем статус... (code 100003).");
                    check_unlock_status(session, cookie_value.as_str(), device_id.as_str())
                        .await;
                }
            }
        }
    } else {
        log("[Статус] Ошибка, заявка отклонена или не подана.");
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Создаем окно
    let window = MainWindow::new()?;

    // Инициализируем логгер
    logger::init(&window);

    // Тест в основном потоке
    logger::log("Программа запустилась!");

    let weak_window = window.as_weak();

    let session = Client::new();

    // Создаем окно AboutPage заранее, но не показываем
    let about = AboutPage::new()?;
    let about_weak = about.as_weak(); // Создаем слабую ссылку
    let about_clone = about_weak.clone();

    about.show()?;

    slint::invoke_from_event_loop({
        let about = about_weak.clone();
        move || {
            // Скрываем окно после одного тика event loop
            about.upgrade().unwrap().hide().unwrap();
        }
    })
    .unwrap();
    // about.hide()?;

    // Обработчик для кнопки "О программе"
    window.on_show_about(move || {
        if let Some(about) = about_clone.upgrade() {
            about.show().unwrap(); // Показываем окно только по нажатию кнопки
        }
    });

    window.on_submit_request(move || {
        if let Some(window) = weak_window.upgrade() {
            let cookie_value = window.get_token();
            let device_id = deviceid::generate_device_id();

            window.set_deviceid(device_id.clone().into());

            if cookie_value.is_empty() {
                log("Ошибка: Cookie пустой".to_string());
                return;
            }

            let session_clone = session.clone();
            let cookie_value_clone = cookie_value.clone();
            let device_id_clone = device_id.clone();

            spawn(async move {
                on_submit(
                    &session_clone,
                    cookie_value_clone.to_string(),
                    device_id_clone,
                )
                .await;
            });
        }
    });


    // Обработчик для гиперссылки в AboutPage
    if let Some(about) = about_weak.upgrade() {
        about.on_hyperlink(move |url| {
            if let Err(e) = open::that(url.to_string()) {
                log(format!("Ошибка открытия URL: {}; {:#?}", url, e));
            };
        });
    }

    window.on_exit(|| std::process::exit(0));

    window.run()?;
    Ok(())
}
