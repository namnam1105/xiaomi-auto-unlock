use crate::logger::{log, update_status};
use std::{
    cmp::min,
    collections::HashMap,
    f64,
    net::{ToSocketAddrs, UdpSocket},
    time::Duration,
};

use chrono::{DateTime, Local, NaiveTime, TimeDelta, TimeZone, Utc};
use chrono_tz::{Asia::Shanghai, Tz};
use reqwest::header::{HeaderMap, HeaderValue};
use serde_json::{Number, Value};
use sntpc::{NtpContext, StdTimestampGen, sync::get_time};
use surge_ping::ping;
use tokio::time::{sleep};

const NTP_SERVERS: [&str; 11] = [
    "time1.google.com:123",
    "time2.google.com:123",
    "time3.google.com:123",
    "time4.google.com:123",
    "time.android.com:123",
    "time.aws.com:123",
    "time.google.com:123",
    "time.cloudflare.com:123",
    "ntp.time.in.ua:123",
    "stratum1.net:123",
    "ntp5.stratum2.ru:123",
];

const MI_SERVERS: [&str; 2] = ["sgp-api.buy.mi.com", "20.157.18.26"];

pub async fn debug_ping(host: &str) -> Option<f64> {
    let addr = match format!("{}:0", host).to_socket_addrs() {
        Ok(mut addrs) => addrs.find(|a| a.is_ipv4())?.ip(),
        Err(_) => return None,
    };

    match tokio::time::timeout(Duration::from_secs(2), ping(addr, &[0; 8])).await {
        Ok(Ok((_, duration))) => Some(duration.as_secs_f64() * 1000.0),
        _ => None,
    }
}

pub async fn get_average_ping() -> f64 {
    let mut all_pings: Vec<f64> = vec![];

    log("Начинаем вычисление пинга...");
    pub async fn ping_server(server: &str) -> Option<f64> {
        let mut pings: Vec<f64> = vec![];
        for attempt in 0..3 {
            match debug_ping(&server).await {
                Some(rtt) => {
                    pings.push(rtt);
                }
                None => {
                    log(format!("Пинг {}/3 не удался", attempt + 1));
                }
            }
            sleep(Duration::from_secs_f64(0.2)).await;
        }
        return if pings.len() > 0 {
            // sum all elements
            let sum: f64 = pings.iter().sum::<f64>();
            let mean = sum / pings.len() as f64;
            Some(mean)
        } else {
            None
        };
    }

    for server in MI_SERVERS {
        if let Some(ping) = ping_server(&server).await {
            all_pings.push(ping);
        } else {
            log(format!("Пинг на {} не удался", server));
        }
    }
    if all_pings.len() == 0 {
        log("Не удалось получить пинг ни до одного сервера!");
        log("Используем значение по умолчанию: 300мс");
        return 150f64;
    } else {
        let sum = all_pings.iter().sum::<f64>();
        let mean = sum / all_pings.len() as f64;
        log("Средний пинг: ".to_string() + mean.to_string().as_str() + "мс");
        mean
    }
}

pub async fn get_initial_beijing_time() -> Option<DateTime<Tz>> {
    // println!("Попытка подключения к NTP-серверу");

    for server in NTP_SERVERS {
        log("Попытка подключения к NTP-серверу: ".to_string() + server);
        if let Ok(mut addrs) = server.to_socket_addrs() {
            if let Some(addr) = addrs.next() {
                let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
                let ntp_context = NtpContext::new(StdTimestampGen::default());
                match get_time(addr, &socket, ntp_context) {
                    Ok(time) => {
                        let unix_time = time.sec() as i64;
                        let datetime_utc = Utc.timestamp_opt(unix_time, 0).single()?;
                        let datetime_beijing =
                            datetime_utc.with_timezone(&chrono_tz::Asia::Shanghai);
                        log("Пекинское время, полученное с сервера ".to_string()
                            + server
                            + ": "
                            + datetime_beijing
                                .format("%Y-%m-%d %H:%M:%S")
                                .to_string()
                                .as_str());
                        return Some(datetime_beijing);
                    }
                    Err(e) => {
                        log(format!("Ошибка подключения к {}: {:?}", server, e));
                    }
                }
            }
        }
    }
    log("Не удалось подключиться ни к одному из NTP серверов.");
    None
}

pub async fn get_synchronized_beijing_time(
    start_beijing_time: DateTime<Tz>,
    start_timestamp: i64,
) -> DateTime<Tz> {
    let elapsed = Local::now().timestamp() - start_timestamp; // Вычитает время start_timestamp из таймстампа нынешнего (UNIX EPOCH)
    let current_time = start_beijing_time + Duration::from_secs(elapsed as u64); // прибавляет к новому времени EPOCH start_beijing_time
    return current_time; // возвращает текущее время в формате DateTime (объекта chrono)
}

fn calculate_script_time(ping: u64) -> f64 {
    return 59.091 + (166 - ping) as f64 * 0.006;
}

pub async fn wait_until_target_time(
    start_beijing_time: DateTime<Tz>,
    start_timestamp: i64,
    ping_delay: u64,
) {
    let script_time = calculate_script_time(ping_delay);
    let seconds = script_time as u32;
    let milliseconds = (script_time % 1f64) * 1000.0;

    let target_time = start_beijing_time.date_naive().and_time(
        NaiveTime::from_hms_micro_opt(23, 59, seconds, milliseconds as u32 * 1000u32).unwrap(),
    );
    let mut target_time = Shanghai.from_local_datetime(&target_time).unwrap();

    let current_time = get_synchronized_beijing_time(start_beijing_time, start_timestamp).await;
    if current_time > target_time {
        log(format!(
            "Текущая дата больше целевой даты, корректируем целевую дату на 1 день"
        ));
        target_time = target_time + chrono::Duration::days(1);
        log(format!("Целевая дата: {}", target_time));
    }

    log(format!(
        "Ожидание до {} (скорректировано по пингу {ping_delay} мс) (Пекинское время)",
        target_time
    ));
    log(format!(
        "Местное время: {}",
        Local.from_utc_datetime(&target_time.naive_utc())
    ));
    loop {
        let current_time: DateTime<Tz> =
            get_synchronized_beijing_time(start_beijing_time, start_timestamp).await;
        let time_difference: TimeDelta = target_time.with_timezone(&Shanghai) - current_time;
        let secs = time_difference.num_seconds() as f64;
        // log(seconds);
        if secs > 0.1f64 {
            let dur = Duration::from_secs_f64(secs * 0.9);
            tokio::time::sleep(dur).await;
        } else if secs > 0.0f64 {
            let dur = Duration::from_secs_f64(secs);
            tokio::time::sleep(dur).await;
        } else if current_time >= target_time {
            log(format!(
                "Время достигнуто: {}. Начинает отправку запросов",
                target_time
            ));
            break;
        }
    }
}

pub async fn check_unlock_status(
    session: &reqwest::Client,
    cookie_value: &str,
    device_id: &str,
) -> bool {
    update_status(false, "Проверка статуса");
    log("Проверяем статус разблокировки...");
    let url = "https://sgp-api.buy.mi.com/bbs/api/global/user/bl-switch/state";

    let mut headers: HeaderMap = HeaderMap::new();
    let header = HeaderValue::from_str(format!("new_bbs_serviceToken={cookie_value};versionCode=500411;versionName=5.4.11;deviceId={device_id};").as_str()).unwrap();
    headers.append("Cookie", header);
    let content_header = HeaderValue::from_str("application/json; charset=utf-8").unwrap();
    headers.append("Content-Type", content_header);

    let response = session.get(url).headers(headers).send().await;
    if let Ok(response) = response {
        log("Ответ получен...");
        let data = response.json::<HashMap<Value, Value>>().await.unwrap();
        // let data: HashMap<String,String> = data.get("data").unwrap();
        // println!("{:?}", data);
        match data.get(&Value::String("code".into())) {
            Some(code) => {
                // log(code);
                if code.as_u64().unwrap() == 100004u64 {
                    update_status(false, "Ошибка");
                    log("Cookie (токен) устарел, обновите. (code 100004)");
                    return false;
                }
            }
            None => {
                log("Ошибка проверки статуса.");
                update_status(false, "Ошибка");
                return false;
            }
        }
        let data = data.get(&Value::String("data".to_string())).unwrap();
        println!("{:?}", data);
        let is_pass = data.get("is_pass").unwrap();
        let button_state = data.get("button_state");
        let deadline_format = data.get("deadline_format");

            let button_state = button_state.unwrap();
            if is_pass == &Value::Number(Number::from(4)) {
                if button_state == &Value::Number(Number::from(1)) {
                    log("[Статус] Аккаунт может подать заявку на разблокировку.");
                    update_status(true, "Можно разблокировать");
                    return true;
                } else if button_state == &Value::Number(Number::from(2)) {
                    log(format!(
                        "[Статус] На аккаунте блокировка на подачу заявки до {} (Месяц/День).",
                        deadline_format.unwrap()
                    ));
                    update_status(false, "Заблокировано");
                    return false;
                } else if button_state == &Value::Number(Number::from(3)) {
                    log("[Статус] Аккаунт создан менее 30 дней назад.");
                    update_status(false, "Менее 30 дней");
                    return false;
                } else if is_pass == &Value::Number(Number::from(1)) {
                    log(format!(
                        "[Статус] Заявка одобрена, разблокировка возможна до {}.",
                        deadline_format.unwrap()
                    ));
                    update_status(true, "Одобрено");
                    return true;

                } else {
                    log("[Статус] Ошибка получения статуса разблокировки.");
                    update_status(false, "Ошибка");
                    return false;
                }
            } else if is_pass == &Value::Number(Number::from(1)) {
                log(format!("[Статус] Заявка одобрена, разблокировка возможна до {}.",deadline_format.unwrap()));
                update_status(true, "Одобрено");
                return true;
            } else {
                log("Ошибка получения ответа.");
                update_status(false, "Ошибка");
                return false;
            }
        } else {
        log("Ошибка получения ответа.");
        update_status(false, "Ошибка");
        return false;
    }
}

pub async fn wait_until_ping_time(start_beijing_time: DateTime<Tz>, start_timestamp: i64) -> f64 {
    let target_time = start_beijing_time
        .date_naive()
        .and_time(NaiveTime::from_hms_micro_opt(23, 59, 48, 0).unwrap());
    let current_time = get_synchronized_beijing_time(start_beijing_time, start_timestamp).await;
    let mut target_time = Shanghai.from_local_datetime(&target_time).unwrap();
    if current_time > target_time {
        log(format!(
            "Текущая дата больше целевой даты, корректируем целевую дату на 1 секунду"
        ));
        target_time = target_time + chrono::Duration::seconds(1);
        log(format!("Целевая дата: {}", target_time));
    }
    log(format!(
        "Ожидание до {} для измерения пинга (Пекинское время)",
        target_time
    ));
    loop {
        let current_time = get_synchronized_beijing_time(start_beijing_time, start_timestamp).await;
        let time_difference: TimeDelta = target_time.with_timezone(&Shanghai) - current_time;
        let secs = time_difference.num_seconds();
        if secs <= 0 {
            log(format!(
                "Время достигнуто: {}. Начинает отправку запросов",
                target_time
            ));
            let avg_ping = get_average_ping().await;
            return avg_ping;
        } else {
            let dur = Duration::from_secs(min(secs as u64, 1));
            tokio::time::sleep(dur).await;
        }
    }
}
