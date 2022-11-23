use super::{sql, tg};

use actix_web::{get, web, HttpResponse};

/// Runtime necessary data.
pub struct State {
    /// connection pool to the sqlite database
    pub db_conn: sqlx::SqlitePool,
    pub token: String,
    pub bot: tg::Bot,
}

/// Alias of the application state data
type Data = actix_web::web::Data<State>;

#[derive(Debug, serde::Serialize)]
enum ReqStatus {
    Ok,
    Fail,
}

/// Default JSON response when some internal error occur. The msg field should contains friendly
/// hint for debugging. And detail field contains the original error.
#[derive(serde::Serialize)]
struct MsgResp {
    status: ReqStatus,
    msg: String,
    detail: String,
}

impl MsgResp {
    fn new_200_msg<D: ToString>(detail: D) -> HttpResponse {
        HttpResponse::Ok().json(Self {
            status: ReqStatus::Ok,
            msg: "Request success".to_string(),
            detail: detail.to_string(),
        })
    }

    /// Create a new Internal Server Error (ise) response
    fn new_500_resp<M, D>(msg: M, detail: D) -> HttpResponse
    where
        M: ToString,
        D: ToString,
    {
        HttpResponse::InternalServerError().json(Self {
            status: ReqStatus::Fail,
            msg: msg.to_string(),
            detail: detail.to_string(),
        })
    }

    fn new_403_resp<M: ToString>(detail: M) -> HttpResponse {
        HttpResponse::Forbidden().json(Self {
            status: ReqStatus::Fail,
            msg: "forbidden".to_string(),
            detail: detail.to_string(),
        })
    }

    fn new_400_resp<M: ToString>(detail: M) -> HttpResponse {
        HttpResponse::BadRequest().json(Self {
            status: ReqStatus::Fail,
            msg: "bad request".to_string(),
            detail: detail.to_string(),
        })
    }
}

#[get("/add")]
pub(super) async fn add() -> HttpResponse {
    todo!()
}

/// Present the JSON response for route `/pkg`.
///
/// The workList contains the package assignment status. And markList contains the marks for each
/// package.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct PkgJsonResponse {
    work_list: Vec<sql::WorkListUnit>,
    mark_list: Vec<sql::MarkListUnit>,
}

/// Implementation of route `/pkg`
#[get("/pkg")]
pub(super) async fn pkg(data: Data) -> HttpResponse {
    let work_list = sql::get_working_list(&data.db_conn).await;
    if let Err(err) = work_list {
        return MsgResp::new_500_resp("fail to get working list", err);
    }

    let mark_list = sql::get_mark_list(&data.db_conn).await;
    if let Err(err) = mark_list {
        return MsgResp::new_500_resp("fail to get mark list", err);
    }

    HttpResponse::Ok().json(PkgJsonResponse {
        work_list: work_list.unwrap(),
        mark_list: mark_list.unwrap(),
    })
}

#[derive(serde::Deserialize)]
pub struct RouteDeletePathSegment {
    pkgname: String,
    status: String,
}

#[derive(serde::Deserialize)]
pub struct RouteDeleteQuery {
    token: String,
}

#[get("/delete/{pkgname}/{status}")]
pub(super) async fn delete(
    path: web::Path<RouteDeletePathSegment>,
    q: web::Query<RouteDeleteQuery>,
    data: Data,
) -> HttpResponse {
    if q.token != data.token {
        return MsgResp::new_403_resp("invalid token");
    }

    if !["ftbfs", "leaf"].contains(&path.status.as_str()) {
        return MsgResp::new_400_resp(format!("Required 'ftbfs' or 'leaf', get {}", path.status));
    }

    let packager = sql::find_packager(
        &data.db_conn,
        sql::FindPackagerProp::ByPkgname(&path.pkgname),
    )
    .await;
    if let Err(err) = packager {
        return MsgResp::new_500_resp("fail to fetch packager", err);
    }
    let packager = packager.unwrap();

    let prefix = "<code>(auto-merge)</code>";
    let text = format!(
        "{prefix} ping {}: {} 已出包",
        tg::gen_mention_link(&packager.alias, packager.tg_uid),
        path.pkgname
    );

    let notify_result = data.bot.send_message(&text).await;
    if let Err(err) = notify_result {
        return MsgResp::new_500_resp("fail to send telegram message", err);
    }

    if let Err(err) = sql::drop_assign(&data.db_conn, &path.pkgname, packager.tg_uid).await {
        let text = format!("{prefix} failed: {err}");
        if let Err(err) = data.bot.send_message(&text).await {
            return MsgResp::new_500_resp("fail to send telegram message", err);
        };
    };

    let mut tasks = Vec::with_capacity(2);
    tasks.push(tokio::spawn(async move {
        // actix_web::Data is just a wrapper for Arc, copy is cheap here.
        let data = data.clone();
        let pkgname = path.pkgname.to_string();
        let matches = &[
            "outdated",
            "stuck",
            "ready",
            "outdated_dep",
            "missing_dep",
            "unknown",
            "ignore",
            "failing",
        ];
        let result = sql::remove_marks(&data.db_conn, &pkgname, Some(matches)).await;
        match result {
            Ok(deleted) => {
                let marks = deleted.join(",");
                data.bot
                    .send_message(&format!(
                        "<code>(auto-unmark)</code> {pkgname} 已出包，不再标记为：{marks}"
                    ))
                    .await
            }
            Err(err) => {
                data.bot
                    .send_message(&format!(
                        "fail to delete marks for {pkgname}: \n<code>{err}</code>"
                    ))
                    .await
            }
        }
    }));

    for t in tasks {
        let result = t.await;
        if let Err(err) = result {
            MsgResp::new_500_resp("Execution fail", err);
        }
    }

    MsgResp::new_200_msg("package deleted")
}
