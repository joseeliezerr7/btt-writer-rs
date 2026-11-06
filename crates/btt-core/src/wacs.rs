//! Cliente de la API Gitea/Gogs de WACS (`content.bibletranslationtools.org`)
//! y DCS (`git.door43.org`): inicio de sesión con token, repositorios y búsqueda.

use crate::sources::http_client;
use crate::{Error, Result};
use reqwest::blocking::{Client, RequestBuilder};
use serde::{Deserialize, Serialize};

pub const WACS_API: &str = "https://content.bibletranslationtools.org/api/v1";
pub const WACS_DEV_API: &str = "https://content.wacsdev.org/api/v1";
pub const DCS_API: &str = "https://git.door43.org/api/v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct User {
    pub id: i64,
    pub username: String,
    #[serde(default)]
    pub full_name: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub avatar_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Token {
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub sha1: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Repo {
    pub id: i64,
    pub name: String,
    pub full_name: String,
    #[serde(default)]
    pub html_url: String,
    #[serde(default)]
    pub clone_url: String,
    #[serde(default)]
    pub ssh_url: String,
    #[serde(default)]
    pub private: bool,
    #[serde(default)]
    pub empty: bool,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub owner: User,
    #[serde(default)]
    pub updated_at: String,
}

impl Repo {
    /// Proyecto que contiene, si el nombre sigue el patrón `{lang}_{book}_{type}[_{res}]`.
    pub fn parsed_id(&self) -> Option<(String, String, String, String)> {
        crate::manifest::parse_unique_id(&self.name)
    }
}

/// Sesión autenticada (se guarda en el perfil).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Session {
    pub user: User,
    pub token: Token,
}

pub struct WacsClient {
    pub api_base: String,
    client: Client,
}

#[derive(Deserialize)]
struct SearchResponse<T> {
    #[serde(default)]
    data: Vec<T>,
}

impl WacsClient {
    pub fn new(api_base: &str) -> Result<Self> {
        Ok(WacsClient { api_base: api_base.trim_end_matches('/').to_string(), client: http_client()? })
    }

    fn url(&self, path: &str) -> String {
        format!("{}/{}", self.api_base, path.trim_start_matches('/'))
    }

    fn send<T: for<'de> Deserialize<'de>>(&self, req: RequestBuilder) -> Result<T> {
        let resp = req.send()?;
        let status = resp.status();
        let body = resp.text()?;
        if status.as_u16() == 401 || status.as_u16() == 403 {
            return Err(Error::Unauthorized);
        }
        if !status.is_success() {
            return Err(Error::Server { status: status.as_u16(), body: body.chars().take(300).collect() });
        }
        if body.trim().is_empty() {
            // Respuestas 204 → valor por defecto vía JSON vacío.
            return serde_json::from_str("null").map_err(Into::into);
        }
        Ok(serde_json::from_str(&body)?)
    }

    fn with_token(&self, req: RequestBuilder, token: &str) -> RequestBuilder {
        req.header("Authorization", format!("token {token}"))
    }

    /// Inicia sesión con usuario/contraseña: valida al usuario, reemplaza el
    /// token con `token_name` si ya existía y crea uno nuevo.
    pub fn login(&self, username: &str, password: &str, token_name: &str) -> Result<Session> {
        let user: User = self.send(self.client.get(self.url("user")).basic_auth(username, Some(password)))?;
        let tokens: Vec<Token> =
            self.send(self.client.get(self.url(&format!("users/{}/tokens", user.username))).basic_auth(username, Some(password)))?;
        for t in tokens.iter().filter(|t| t.name == token_name) {
            let _: Option<serde_json::Value> = self
                .send(
                    self.client
                        .delete(self.url(&format!("users/{}/tokens/{}", user.username, t.id)))
                        .basic_auth(username, Some(password)),
                )
                .ok();
        }
        let token: Token = self.send(
            self.client
                .post(self.url(&format!("users/{}/tokens", user.username)))
                .basic_auth(username, Some(password))
                .json(&serde_json::json!({ "name": token_name, "scopes": ["write:repository", "write:user"] })),
        )?;
        if token.sha1.is_empty() {
            return Err(Error::Other("el servidor no devolvió el token".into()));
        }
        Ok(Session { user, token })
    }

    /// Verifica que un token sigue siendo válido y devuelve el usuario.
    pub fn current_user(&self, token: &str) -> Result<User> {
        self.send(self.with_token(self.client.get(self.url("user")), token))
    }

    /// Borra el token de la sesión (cerrar sesión). Requiere contraseña o token
    /// con permisos; si falla se ignora.
    pub fn logout(&self, session: &Session) -> Result<()> {
        let _: Option<serde_json::Value> = self
            .send(self.with_token(
                self.client.delete(self.url(&format!("users/{}/tokens/{}", session.user.username, session.token.id))),
                &session.token.sha1,
            ))
            .ok();
        Ok(())
    }

    pub fn get_repo(&self, token: Option<&str>, owner: &str, name: &str) -> Result<Repo> {
        let mut req = self.client.get(self.url(&format!("repos/{owner}/{name}")));
        if let Some(t) = token {
            req = self.with_token(req, t);
        }
        self.send(req)
    }

    /// Crea el repositorio del usuario; si ya existe (409) lo devuelve.
    pub fn create_repo(&self, token: &str, username: &str, name: &str) -> Result<Repo> {
        let req = self.with_token(self.client.post(self.url("user/repos")), token).json(&serde_json::json!({
            "name": name,
            "description": format!("btt-writer: {name}"),
            "private": false,
            "auto_init": false,
        }));
        match self.send::<Repo>(req) {
            Ok(r) => Ok(r),
            Err(Error::Server { status: 409, .. }) => self.get_repo(Some(token), username, name),
            Err(e) => Err(e),
        }
    }

    /// Busca repositorios. `uid` limita a un usuario.
    pub fn search_repos(&self, query: &str, uid: Option<i64>, limit: usize) -> Result<Vec<Repo>> {
        let q = if query.trim().is_empty() { "_" } else { query.trim() };
        let mut req = self.client.get(self.url("repos/search")).query(&[("q", q), ("limit", &limit.to_string())]);
        if let Some(u) = uid {
            req = req.query(&[("uid", u.to_string())]);
        }
        let r: SearchResponse<Repo> = self.send(req)?;
        Ok(r.data)
    }

    pub fn search_users(&self, query: &str, limit: usize) -> Result<Vec<User>> {
        let r: SearchResponse<User> =
            self.send(self.client.get(self.url("users/search")).query(&[("q", query.trim()), ("limit", &limit.to_string())]))?;
        Ok(r.data)
    }

    /// Repositorios a los que el usuario tiene acceso, incluidos los que otros le compartieron.
    pub fn my_repos(&self, token: &str) -> Result<Vec<Repo>> {
        let mut out = vec![];
        for page in 1..=5 {
            let r: Vec<Repo> = self.send(self.with_token(
                self.client.get(self.url("user/repos")).query(&[("limit", "50"), ("page", &page.to_string())]),
                token,
            ))?;
            let n = r.len();
            out.extend(r);
            if n < 50 {
                break;
            }
        }
        Ok(out)
    }

    /// Colaboradores de un repositorio (sin el dueño).
    pub fn collaborators(&self, token: &str, owner: &str, repo: &str) -> Result<Vec<User>> {
        self.send(self.with_token(self.client.get(self.url(&format!("repos/{owner}/{repo}/collaborators"))), token))
    }

    /// Da permiso de escritura a `username` sobre el repositorio.
    pub fn add_collaborator(&self, token: &str, owner: &str, repo: &str, username: &str) -> Result<()> {
        let _: Option<serde_json::Value> = self.send(
            self.with_token(self.client.put(self.url(&format!("repos/{owner}/{repo}/collaborators/{username}"))), token)
                .json(&serde_json::json!({ "permission": "write" })),
        )?;
        Ok(())
    }

    pub fn remove_collaborator(&self, token: &str, owner: &str, repo: &str, username: &str) -> Result<()> {
        let _: Option<serde_json::Value> =
            self.send(self.with_token(self.client.delete(self.url(&format!("repos/{owner}/{repo}/collaborators/{username}"))), token))?;
        Ok(())
    }

    /// Repositorios de un usuario, filtrados por texto (proyecto o idioma).
    pub fn user_repos(&self, username: &str, query: &str) -> Result<Vec<Repo>> {
        let users = self.search_users(username, 5)?;
        let Some(u) = users.into_iter().find(|u| u.username.eq_ignore_ascii_case(username)) else {
            return Ok(vec![]);
        };
        let mut repos = self.search_repos(query, Some(u.id), 100)?;
        repos.retain(|r| r.owner.username.eq_ignore_ascii_case(username) || r.full_name.to_lowercase().starts_with(&format!("{}/", username.to_lowercase())));
        Ok(repos)
    }
}

/// URL pública de lectura de un proyecto subido.
pub fn reader_url(reader_base: &str, username: &str, project_id: &str) -> String {
    format!("{}/{}/{}", reader_base.trim_end_matches('/'), username, project_id)
}

/// Nombre del token por dispositivo (igual que 2.0: `btt-writer__{device}`).
pub fn token_name(device: &str) -> String {
    let d: String = device.chars().map(|c| if c.is_alphanumeric() { c } else { '_' }).collect();
    format!("btt-writer-rs__{d}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_real_search_payload() {
        let json = r#"{"ok":true,"data":[{"id":3876,"owner":{"id":145,"login":"PlateauMalagasy","full_name":"","email":"x@y","username":"PlateauMalagasy"},"name":"-x-antambahoaka_3jn_text_reg","full_name":"PlateauMalagasy/-x-antambahoaka_3jn_text_reg","description":"ts-desktop","empty":false,"private":false,"html_url":"https://content.bibletranslationtools.org/PlateauMalagasy/-x-antambahoaka_3jn_text_reg","ssh_url":"git@content-ssh.bibletranslationtools.org:PlateauMalagasy/-x-antambahoaka_3jn_text_reg.git","clone_url":"https://content.bibletranslationtools.org/PlateauMalagasy/-x-antambahoaka_3jn_text_reg.git","updated_at":"2020-01-01T00:00:00Z"}]}"#;
        let r: SearchResponse<Repo> = serde_json::from_str(json).unwrap();
        assert_eq!(r.data.len(), 1);
        assert_eq!(r.data[0].owner.username, "PlateauMalagasy");
        let (lang, book, t, res) = r.data[0].parsed_id().unwrap();
        assert_eq!((lang.as_str(), book.as_str(), t.as_str(), res.as_str()), ("-x-antambahoaka", "3jn", "text", "reg"));
    }

    #[test]
    fn helper_urls() {
        assert_eq!(reader_url("https://read.bibleineverylanguage.org/", "jose", "es_3jn_text_reg"), "https://read.bibleineverylanguage.org/jose/es_3jn_text_reg");
        assert_eq!(token_name("RTX 5060TI"), "btt-writer-rs__RTX_5060TI");
        let c = WacsClient::new("https://x/api/v1/").unwrap();
        assert_eq!(c.url("/user"), "https://x/api/v1/user");
    }
}
