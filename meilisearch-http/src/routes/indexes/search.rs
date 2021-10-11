use actix_web::{web, HttpRequest, HttpResponse};
use http::header::USER_AGENT;
use log::debug;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::analytics::Analytics;
use crate::error::ResponseError;
use crate::extractors::authentication::{policies::*, GuardedData};
use crate::index::{default_crop_length, SearchQuery, DEFAULT_SEARCH_LIMIT};
use crate::routes::IndexParam;
use crate::Data;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("")
            .route(web::get().to(search_with_url_query))
            .route(web::post().to(search_with_post)),
    );
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchQueryGet {
    q: Option<String>,
    offset: Option<usize>,
    limit: Option<usize>,
    attributes_to_retrieve: Option<String>,
    attributes_to_crop: Option<String>,
    #[serde(default = "default_crop_length")]
    crop_length: usize,
    attributes_to_highlight: Option<String>,
    filter: Option<String>,
    sort: Option<String>,
    #[serde(default = "Default::default")]
    matches: bool,
    facets_distribution: Option<String>,
}

impl From<SearchQueryGet> for SearchQuery {
    fn from(other: SearchQueryGet) -> Self {
        let attributes_to_retrieve = other
            .attributes_to_retrieve
            .map(|attrs| attrs.split(',').map(String::from).collect());

        let attributes_to_crop = other
            .attributes_to_crop
            .map(|attrs| attrs.split(',').map(String::from).collect());

        let attributes_to_highlight = other
            .attributes_to_highlight
            .map(|attrs| attrs.split(',').map(String::from).collect());

        let facets_distribution = other
            .facets_distribution
            .map(|attrs| attrs.split(',').map(String::from).collect());

        let filter = match other.filter {
            Some(f) => match serde_json::from_str(&f) {
                Ok(v) => Some(v),
                _ => Some(Value::String(f)),
            },
            None => None,
        };

        let sort = other
            .sort
            .map(|attrs| attrs.split(',').map(String::from).collect());

        Self {
            q: other.q,
            offset: other.offset,
            limit: other.limit.unwrap_or(DEFAULT_SEARCH_LIMIT),
            attributes_to_retrieve,
            attributes_to_crop,
            crop_length: other.crop_length,
            attributes_to_highlight,
            filter,
            sort,
            matches: other.matches,
            facets_distribution,
        }
    }
}

pub async fn search_with_url_query(
    req: HttpRequest,
    data: GuardedData<Public, Data>,
    path: web::Path<IndexParam>,
    params: web::Query<SearchQueryGet>,
    analytics: web::Data<&'static Analytics>,
) -> Result<HttpResponse, ResponseError> {
    debug!("called with params: {:?}", params);
    let query = params.into_inner().into();
    let search_result = data.search(path.into_inner().index_uid, query).await?;

    // Tests that the nb_hits is always set to false
    #[cfg(test)]
    assert!(!search_result.exhaustive_nb_hits);

    analytics.publish(
        "Search get".to_string(),
        json!({
                "user-agent": req.headers().get(USER_AGENT).map(|header| header.to_str().unwrap_or_default()).unwrap_or_default(),
        }),
    );

    debug!("returns: {:?}", search_result);
    Ok(HttpResponse::Ok().json(search_result))
}

pub async fn search_with_post(
    req: HttpRequest,
    data: GuardedData<Public, Data>,
    path: web::Path<IndexParam>,
    params: web::Json<SearchQuery>,
    analytics: web::Data<&'static Analytics>,
) -> Result<HttpResponse, ResponseError> {
    debug!("search called with params: {:?}", params);

    analytics.publish(
        "Search post".to_string(),
        json!({
                "sort": params.sort.as_ref().map(|vec| vec.len()).unwrap_or_default(),
                "user-agent": req.headers().get(USER_AGENT).map(|header| header.to_str().unwrap_or_default()).unwrap_or_default(),
        }),
    );

    let search_result = data
        .search(path.into_inner().index_uid, params.into_inner())
        .await?;

    // Tests that the nb_hits is always set to false
    #[cfg(test)]
    assert!(!search_result.exhaustive_nb_hits);

    debug!("returns: {:?}", search_result);
    Ok(HttpResponse::Ok().json(search_result))
}
