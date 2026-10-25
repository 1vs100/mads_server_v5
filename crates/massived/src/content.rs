//! Zone and asset content built from the configuration.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use massive_proto::crypto::AssetDigest;
use massive_proto::ids::{
    AngleThreshold, AssetId, AssetType, AssetUrl, ByteCount, CrexId, DistanceThreshold, Expiration,
    FrequencyCap, IeId, IeName, InvalidText, MediaType, OrderId, RotationDuration, RotationType,
    SizeThreshold, ZoneName,
};
use massive_proto::response::{AssetRecord, EnterZoneResponse, MaoRecord, Order};
use thiserror::Error;
use tracing::warn;

use crate::config::{AssetConfig, Config, ZoneConfig};

#[derive(Debug, Error)]
pub enum ContentError {
    #[error("{what} {value:?} is not valid: {source}")]
    Text {
        what: &'static str,
        value: String,
        source: InvalidText,
    },
    #[error("asset {asset_id} is defined more than once")]
    DuplicateAsset { asset_id: u32 },
    #[error("cannot read asset {asset_id} from {path}: {source}")]
    AssetFile {
        asset_id: u32,
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("asset {asset_id} file {path} is {len} bytes, more than a u32 length allows")]
    AssetTooLarge {
        asset_id: u32,
        path: PathBuf,
        len: usize,
    },
    #[error("asset {asset_id} would make the client report error 0x{code:X}")]
    AssetFault { asset_id: u32, code: u16 },
    #[error("URL template {template:?} contains the unknown placeholder {placeholder:?}")]
    UrlTemplate {
        template: String,
        placeholder: String,
    },
    #[error("zone {zone:?} is defined more than once")]
    DuplicateZone { zone: String },
    #[error("zone {zone:?}: {what} {value} is used more than once")]
    DuplicateInZone {
        zone: String,
        what: &'static str,
        value: String,
    },
    #[error(
        "zone {zone:?}: placement {ie_name:?} has media type 0x{media_type:X}, which selects no object class"
    )]
    MediaType {
        zone: String,
        ie_name: String,
        media_type: u32,
    },
    #[error("zone {zone:?}: {referrer} refers to undefined asset {asset_id}")]
    UnknownAsset {
        zone: String,
        referrer: String,
        asset_id: u32,
    },
    #[error("two assets resolve to the same URL path {path:?}")]
    DuplicatePath { path: String },
}

#[derive(Debug)]
pub struct StoredAsset {
    pub id: AssetId,
    pub file: PathBuf,
    pub url: AssetUrl,
    pub body: Arc<[u8]>,
}

#[derive(Debug, Default)]
pub struct Content {
    zones: HashMap<ZoneName, EnterZoneResponse>,
    announced: Vec<ZoneName>,
    assets_by_path: HashMap<String, Arc<StoredAsset>>,
}

impl Content {
    pub fn load(config: &Config) -> Result<Self, ContentError> {
        let mut records = HashMap::new();
        let mut assets_by_path = HashMap::new();
        for asset in &config.assets {
            let (record, stored) = load_asset(config, asset)?;
            let path = url_path(stored.url.as_str()).to_owned();
            if records.insert(asset.id, record).is_some() {
                return Err(ContentError::DuplicateAsset { asset_id: asset.id });
            }
            if assets_by_path
                .insert(path.clone(), Arc::new(stored))
                .is_some()
            {
                return Err(ContentError::DuplicatePath { path });
            }
        }
        let mut zones = HashMap::new();
        for zone in &config.zones {
            let name = text("zone name", &zone.name, ZoneName::new)?;
            let response = build_zone(zone, &records)?;
            if zones.insert(name, response).is_some() {
                return Err(ContentError::DuplicateZone {
                    zone: zone.name.clone(),
                });
            }
        }
        let announced = announced_zones(&config.content.announce_zones, &zones)?;
        Ok(Self {
            zones,
            announced,
            assets_by_path,
        })
    }

    #[must_use]
    pub fn zone(&self, name: &ZoneName) -> Option<&EnterZoneResponse> {
        self.zones.get(name)
    }

    #[must_use]
    pub fn asset_by_path(&self, path: &str) -> Option<&Arc<StoredAsset>> {
        self.assets_by_path.get(path)
    }

    /// Zones with configured content.
    #[must_use]
    pub fn zone_count(&self) -> usize {
        self.zones.len()
    }

    /// Zones for the OpenSession response.
    #[must_use]
    pub fn announced_zones(&self) -> &[ZoneName] {
        &self.announced
    }

    #[must_use]
    pub fn asset_count(&self) -> usize {
        self.assets_by_path.len()
    }
}

fn text<T>(
    what: &'static str,
    value: &str,
    ctor: impl FnOnce(String) -> Result<T, InvalidText>,
) -> Result<T, ContentError> {
    ctor(value.to_owned()).map_err(|source| ContentError::Text {
        what,
        value: value.to_owned(),
        source,
    })
}

/// Path part of an absolute `http://host[:port]/path` URL, or the input.
#[must_use]
pub fn url_path(url: &str) -> &str {
    url.strip_prefix("http://")
        .and_then(|rest| rest.find('/').map(|i| &rest[i..]))
        .unwrap_or(url)
}

fn render_url(config: &Config, asset_id: u32) -> Result<String, ContentError> {
    let template = &config.content.url_template;
    let mut out = String::new();
    let mut rest = template.as_str();
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let end = after.find('}').ok_or_else(|| ContentError::UrlTemplate {
            template: template.clone(),
            placeholder: after.to_owned(),
        })?;
        match &after[..end] {
            "host" => out.push_str(&config.server.advertise_host),
            "port" => out.push_str(&config.server.advertise_port.to_string()),
            "asset_id" => out.push_str(&asset_id.to_string()),
            other => {
                return Err(ContentError::UrlTemplate {
                    template: template.clone(),
                    placeholder: other.to_owned(),
                });
            }
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

fn load_asset(
    config: &Config,
    asset: &AssetConfig,
) -> Result<(AssetRecord, StoredAsset), ContentError> {
    let file = config.content.directory.join(&asset.file);
    let body = std::fs::read(&file).map_err(|source| ContentError::AssetFile {
        asset_id: asset.id,
        path: file.clone(),
        source,
    })?;
    let size = u32::try_from(body.len()).map_err(|_| ContentError::AssetTooLarge {
        asset_id: asset.id,
        path: file.clone(),
        len: body.len(),
    })?;
    let url = text("asset URL", &render_url(config, asset.id)?, AssetUrl::new)?;
    let record = AssetRecord {
        asset_id: AssetId(asset.id),
        expiration: Expiration(asset.expiration),
        digest: AssetDigest::of(&body),
        rotation_duration: RotationDuration(asset.rotation_duration),
        size: ByteCount(size),
        asset_type: AssetType(asset.asset_type),
        url: url.clone(),
        crex_id: CrexId(asset.crex_id),
        angle_threshold: asset.angle_threshold.map(AngleThreshold),
        distance_threshold: asset.distance_threshold.map(DistanceThreshold),
        size_threshold: asset.size_threshold.map(SizeThreshold),
        extra: asset.extra.clone(),
    };
    if let Some(fault) = record.faults().first() {
        return Err(ContentError::AssetFault {
            asset_id: asset.id,
            code: fault.code(),
        });
    }
    let stored = StoredAsset {
        id: AssetId(asset.id),
        file,
        url,
        body: body.into(),
    };
    Ok((record, stored))
}

fn build_zone(
    zone: &ZoneConfig,
    records: &HashMap<u32, AssetRecord>,
) -> Result<EnterZoneResponse, ContentError> {
    let lookup = |referrer: String, id: u32| {
        records.get(&id).ok_or_else(|| ContentError::UnknownAsset {
            zone: zone.name.clone(),
            referrer,
            asset_id: id,
        })
    };
    let duplicate = |what: &'static str, value: String| ContentError::DuplicateInZone {
        zone: zone.name.clone(),
        what,
        value,
    };

    let mut orders = Vec::new();
    let mut order_ids = HashSet::new();
    let mut ordered_assets = HashSet::new();
    for order in &zone.orders {
        if !order_ids.insert(order.order_id) {
            return Err(duplicate("order id", order.order_id.to_string()));
        }
        let assets = order
            .assets
            .iter()
            .map(|&id| {
                ordered_assets.insert(id);
                lookup(format!("order {}", order.order_id), id).cloned()
            })
            .collect::<Result<_, _>>()?;
        orders.push(Order {
            order_id: OrderId(order.order_id),
            frequency_cap: order.frequency_cap.map(FrequencyCap),
            assets,
        });
    }

    let mut maos = Vec::new();
    let mut names = HashSet::new();
    let mut ie_ids = HashSet::new();
    for p in &zone.placements {
        if !names.insert(p.ie_name.as_str()) {
            return Err(duplicate("IE name", p.ie_name.clone()));
        }
        if !ie_ids.insert(p.ie_id) {
            return Err(duplicate("IE id", p.ie_id.to_string()));
        }
        let media_type = MediaType(p.media_type);
        if media_type.class().is_none() {
            return Err(ContentError::MediaType {
                zone: zone.name.clone(),
                ie_name: p.ie_name.clone(),
                media_type: p.media_type,
            });
        }
        for &id in &p.assets {
            lookup(format!("placement {}", p.ie_name), id)?;
            if !ordered_assets.contains(&id) {
                warn!(
                    zone = %zone.name,
                    ie_name = %p.ie_name,
                    asset_id = id,
                    "placement refers to an asset that no order of the zone carries"
                );
            }
        }
        maos.push(MaoRecord {
            ie_id: IeId(p.ie_id),
            ie_name: text("IE name", &p.ie_name, IeName::new)?,
            media_type,
            rotation_type: RotationType(p.rotation_type),
            primary_match_list_size: p.primary_match_list_size,
            asset_ids: p.assets.iter().copied().map(AssetId).collect(),
            angle_threshold: p.angle_threshold.map(AngleThreshold),
            distance_threshold: p.distance_threshold.map(DistanceThreshold),
            size_threshold: p.size_threshold.map(SizeThreshold),
            adjustment: None,
        });
    }
    Ok(EnterZoneResponse { maos, orders })
}

fn announced_zones(
    names: &[String],
    configured: &HashMap<ZoneName, EnterZoneResponse>,
) -> Result<Vec<ZoneName>, ContentError> {
    let mut announced = Vec::with_capacity(names.len() + configured.len());
    for name in names {
        let zone = text("announced zone name", name, ZoneName::new)?;
        if announced.contains(&zone) {
            return Err(ContentError::DuplicateZone { zone: name.clone() });
        }
        announced.push(zone);
    }
    let mut extra: Vec<&ZoneName> = configured
        .keys()
        .filter(|z| !announced.contains(z))
        .collect();
    extra.sort();
    announced.extend(extra.into_iter().cloned());
    Ok(announced)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty() -> EnterZoneResponse {
        EnterZoneResponse {
            maos: Vec::new(),
            orders: Vec::new(),
        }
    }

    #[test]
    fn announced_zones_append_configured_ones() {
        let configured = HashMap::from([
            (ZoneName::new("Lobby_PT").unwrap(), empty()),
            (ZoneName::new("Extra").unwrap(), empty()),
        ]);
        let names = vec!["Lobby_PT".to_owned(), "Lobby_EP".to_owned()];
        let announced = announced_zones(&names, &configured).unwrap();
        let announced: Vec<&str> = announced.iter().map(ZoneName::as_str).collect();
        assert_eq!(announced, ["Lobby_PT", "Lobby_EP", "Extra"]);
    }

    #[test]
    fn duplicate_announced_zone_is_rejected() {
        let names = vec!["Lobby_PT".to_owned(), "Lobby_PT".to_owned()];
        assert!(matches!(
            announced_zones(&names, &HashMap::new()),
            Err(ContentError::DuplicateZone { .. })
        ));
    }

    #[test]
    fn url_path_strips_scheme_and_authority() {
        assert_eq!(url_path("http://10.0.0.1:80/assets/1"), "/assets/1");
        assert_eq!(url_path("/assets/1"), "/assets/1");
        assert_eq!(url_path("http://host"), "http://host");
    }
}
