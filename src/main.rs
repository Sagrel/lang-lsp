use std::collections::HashMap;

extern crate lang_frontend;
use dashmap::DashMap;

mod hover;
mod inlay_hints;
mod semantic_tokens;
use inlay_hints::get_inlay_hints;
use lang_frontend::ast::{Ast, Spanned};
use lang_frontend::inferer::Inferer;
use lang_frontend::tokenizer::{Span, Token};
use lang_frontend::types::Type;
use lang_frontend::*;
use ropey::Rope;
use semantic_tokens::*;
use serde::{Deserialize, Serialize};

use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::notification::Notification;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

#[derive(Debug)]
struct Backend {
    // Una referencia al cliente que nos permite pasarle mensajes
    client: Client,
    // Un HashMap de Path -> Modulo
    ast_map: DashMap<String, (Vec<Spanned<Ast>>, Vec<Type>)>,
    // Un HashMap de Path -> Source (Rope es un String que se puede modificar rapido)
    document_map: DashMap<String, Rope>,
    // Un HashMap de Path -> Lista de Tokens
    token_map: DashMap<String, Vec<(Token, Span)>>,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    // Especifica que cosas puede hacer nuesto LSP
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            server_info: None,
            capabilities: ServerCapabilities {
                // Quiero sincronizar todo el texto
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                // Configuramos los colorcitos de los tokens
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensRegistrationOptions(
                        SemanticTokensRegistrationOptions {
                            text_document_registration_options: {
                                TextDocumentRegistrationOptions {
                                    // Queremos que funcione en los archivos terminados en "language"?
                                    document_selector: Some(vec![DocumentFilter {
                                        language: Some("lang".to_string()),
                                        scheme: Some("file".to_string()),
                                        pattern: None,
                                    }]),
                                }
                            },
                            // Decimos los tipos de tokens que vamos a proporcionar
                            // Tambien decimos que somos capaces de tokenizar parcial y totalmente un archivo
                            semantic_tokens_options: SemanticTokensOptions {
                                work_done_progress_options: WorkDoneProgressOptions::default(),
                                legend: SemanticTokensLegend {
                                    token_types: Vec::from(LEGEND_TYPE),
                                    token_modifiers: vec![],
                                },
                                range: Some(false),
                                full: Some(SemanticTokensFullOptions::Bool(true)),
                            },
                            static_registration_options: StaticRegistrationOptions::default(),
                        },
                    ),
                ),
                // Estas son las otras capacidades que tiene nuesto servidor
                ..ServerCapabilities::default()
            },
        })
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let params = params.text_document_position_params;
        // El path
        let uri = params.text_document.uri.to_string();
        // Un pequeño mensaje al cliente
        self.client.log_message(MessageType::LOG, "hovering").await;

        let rope = if let Some(entry) = self.document_map.get(&uri) {
            entry.value().clone()
        } else {
            return Ok(None);
        };

        let (ast, type_table) = if let Some(entry) = self.ast_map.get(&uri) {
            entry.value().clone() // SPEED dont clone
        } else {
            return Ok(None);
        };

        let pos = params.position;

        let char = rope.try_line_to_char(pos.line as usize).unwrap_or(0);
        let offset = char + pos.character as usize;

        for declaration in ast.iter() {
            if let Some(t) = hover::find_match(declaration, offset) {
                return Ok(Some(Hover {
                    contents: HoverContents::Scalar(MarkedString::String(format!(
                        "Type: {}",
                        Inferer::get_most_concrete_type(&t, &type_table)
                    ))),
                    range: None,
                }));
            }
        }

        Ok(None)
    }

    // Genera una lista de Token dado un Path
    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        // El path
        let uri = params.text_document.uri.to_string();
        // Un pequeño mensaje al cliente
        self.client
            .log_message(MessageType::LOG, "semantic_token_full")
            .await;

        // Aqui es donde creamos los SemanticToken, que son basicamente la posicion y el tipo de Token

        // Cargamos el texto del archivo que nos diga el path
        let rope = self.document_map.get(&uri).unwrap();
        // Generamos los tokens del archivo
        //let tokens = tokenizer::tokenizer().parse(rope.to_string()).unwrap();
        let tokens = self.token_map.get(&uri).expect("Fuck, we have no tokens");
        // Transformamos los (Token, Span) en SemanticToken
        let semantic_tokens = make_tokens_semantic(&tokens, &rope);

        Ok(Some(SemanticTokensResult::Tokens(SemanticTokens {
            result_id: None,
            data: semantic_tokens,
        })))
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "initialized!")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_change_workspace_folders(&self, _: DidChangeWorkspaceFoldersParams) {
        self.client
            .log_message(MessageType::INFO, "workspace folders changed!")
            .await;
    }

    async fn did_change_configuration(&self, _: DidChangeConfigurationParams) {
        self.client
            .log_message(MessageType::INFO, "configuration changed!")
            .await;
    }

    async fn did_change_watched_files(&self, _: DidChangeWatchedFilesParams) {
        self.client
            .log_message(MessageType::INFO, "watched files have changed!")
            .await;
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.client
            .log_message(MessageType::INFO, "file opened!")
            .await;
        self.on_change(TextDocumentItem {
            uri: params.text_document.uri,
            text: params.text_document.text,
            version: params.text_document.version,
        })
        .await
    }

    async fn did_change(&self, mut params: DidChangeTextDocumentParams) {
        self.on_change(TextDocumentItem {
            uri: params.text_document.uri,
            text: std::mem::take(&mut params.content_changes[0].text),
            version: params.text_document.version,
        })
        .await
    }

    async fn did_save(&self, _: DidSaveTextDocumentParams) {
        self.client
            .log_message(MessageType::INFO, "file saved!")
            .await;
    }

    async fn did_close(&self, _: DidCloseTextDocumentParams) {
        self.client
            .log_message(MessageType::INFO, "file closed!")
            .await;
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct InlayHintParams {
    path: String,
}

enum CustomNotification {}
impl Notification for CustomNotification {
    type Params = InlayHintParams;
    const METHOD: &'static str = "custom/notification";
}
struct TextDocumentItem {
    uri: Url,
    text: String,
    version: i32,
}
impl Backend {
    // TODO why does it only work after we modify the code the first time?
    async fn inlay_hint(&self, params: InlayHintParams) -> Result<Vec<(usize, usize, String)>> {
        let mut hints = HashMap::new();
        if let Some(entry) = self.ast_map.get(&params.path) {
            let ast = &entry.0;
            let type_table = &entry.1;

            for node in ast {
                get_inlay_hints(node, &mut hints);
            }
            let inlay_hint_list = hints
                .into_iter()
                .map(|(k, t)| {
                    (
                        k.start,
                        k.start + 1,
                        Inferer::get_most_concrete_type(&t, type_table).to_string(),
                    )
                })
                .collect::<Vec<_>>();
            Ok(inlay_hint_list)
        } else {
            Ok(Vec::new())
        }
    }

    // TODO be more error resilient to fucked AST
    async fn on_change(&self, params: TextDocumentItem) {
        // Añadimos el contenido del archivo a nuestro document_map
        let rope = ropey::Rope::from_str(&params.text);
        self.document_map
            .insert(params.uri.to_string(), rope.clone());

        // Compilamos el archivo
        let (tokens, ast_and_type_table, errors) = parse_file(params.text.as_str());

        // Transformamos nuestros errores en diagnosticos que VS Code puede usar
        let diagnostics = errors
            .into_iter()
            .map(|item| {
                let (message, span) = match item.reason() {
                    chumsky::error::SimpleReason::Unclosed { span, delimiter } => {
                        (format!("Unclosed delimiter {}", delimiter), span.clone())
                    }
                    chumsky::error::SimpleReason::Unexpected => (
                        format!(
                            "{}, expected {}",
                            if item.found().is_some() {
                                "Unexpected token in input"
                            } else {
                                "Unexpected end of input"
                            },
                            if item.expected().len() == 0 {
                                "something else".to_string()
                            } else {
                                item.expected()
                                    .map(|expected| match expected {
                                        Some(expected) => expected.to_string(),
                                        None => "end of input".to_string(),
                                    })
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            }
                        ),
                        item.span(),
                    ),
                    chumsky::error::SimpleReason::Custom(msg) => (msg.to_string(), item.span()),
                };

                let start_position = offset_to_position(span.start, &rope).unwrap();
                let end_position = offset_to_position(span.end, &rope).unwrap();

                Diagnostic::new_simple(Range::new(start_position, end_position), message)
            })
            .collect::<Vec<_>>();

        // Enviamos los diagnosticos
        self.client
            .publish_diagnostics(params.uri.clone(), diagnostics, Some(params.version))
            .await;

        if let Some(ast_and_type_table) = ast_and_type_table {
            self.ast_map
                .insert(params.uri.to_string(), ast_and_type_table);
        }

        if let Some(tokens) = tokens {
            self.token_map.insert(params.uri.to_string(), tokens);
        }
    }
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    // Creo el server e inicialido el Backend
    let (service, socket) = LspService::build(|client| Backend {
        client,
        ast_map: DashMap::new(),
        document_map: DashMap::new(),
        token_map: DashMap::new(),
    })
    // Añado un metodo que se llama inlay_hit, esto es lo que hace que aparezcan tipos en las variables
    .custom_method("custom/inlay_hint", Backend::inlay_hint)
    .finish();
    Server::new(stdin, stdout, socket).serve(service).await;
}

// TODO I don't like this, can we not use ropes? I don't think we are using them right any way
fn offset_to_position(offset: usize, rope: &Rope) -> Option<Position> {
    let line = rope.try_char_to_line(offset).ok()?;
    let first_char = rope.try_line_to_char(line).ok()?;
    let column = offset - first_char;
    Some(Position::new(line as u32, column as u32))
}
