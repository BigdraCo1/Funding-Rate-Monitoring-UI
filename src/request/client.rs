use crate::third_party::lighter::{api_path::LIGHTER_FUNDING_RATE_API, data::*};
use hyperliquid_rust_sdk::{BaseUrl, InfoClient, Meta};

use reqwest::get;

pub async fn coin_list_metadata() -> anyhow::Result<Meta> {
    let client = InfoClient::new(None, Some(BaseUrl::Mainnet))
        .await
        .expect("Failed to create client");

    let info = client.meta().await.expect("Failed to get meta");

    Ok(info)
}

pub async fn coin_list_metadate_lighter() -> anyhow::Result<Vec<FundingRate>> {
    let response = get(LIGHTER_FUNDING_RATE_API).await?.text().await?;
    let parse_json: ApiFundingRatesResponse = serde_json::from_str(&response)?;
    if parse_json.code != 200 {
        return Err(anyhow::anyhow!("Failed to get funding rates"));
    }
    let mut funding_rates = parse_json.funding_rates;
    funding_rates.dedup_by_key(|c| c.market_id);
    funding_rates.sort_by(|a, b| a.market_id.cmp(&b.market_id));
    Ok(funding_rates)
}
