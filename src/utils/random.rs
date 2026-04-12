use serenity::all::Timestamp;
use tokio::sync::Mutex;

static COUNTER: Mutex<u64> = Mutex::const_new(0);

pub async fn random() -> u64 {
    let mut state = COUNTER.lock().await;

    if *state == 0 {
        *state = Timestamp::now().timestamp() as u64;
    }

    *state ^= *state >> 12;
    *state ^= *state << 25;
    *state ^= *state >> 27;
    *state = state.wrapping_mul(2685821657736338717);
    *state
}

const CHAR_MAP: &str = "ABCDEFGHJKLMNPRSTUVWXYZabcdefghjkmnpqrstuvwxyz123456789";

pub async fn tinyid() -> String {
    let mut res = String::new();

    for _ in 1..=6 {
        let rand = (random().await % CHAR_MAP.len() as u64) as usize;
        res.push(CHAR_MAP.chars().nth(rand).unwrap());
    }

    res
}
