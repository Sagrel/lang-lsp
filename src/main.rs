use std::collections::HashMap;

extern crate lang;
use chumsky::Parser;
use dashmap::DashMap;
use lang::*;

mod semantic_tokens;
use lang::ast::{Ast, Spanned};
use lang::inferer::Type;
use lang::tokenizer::{Span, Token};
use ropey::Rope;
use semantic_tokens::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tower_lsp::jsonrpc::{Result, self, ErrorCode};
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
                // Quiero proporcionar autocompletado cuando de pulse .
                /*
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: Some(vec![".".to_string()]),
                    work_done_progress_options: Default::default(),
                    all_commit_characters: None,
                }),
                */
                // Ni idea TODO probar a quitarlo a ver que pasa
                /*
                execute_command_provider: Some(ExecuteCommandOptions {
                    commands: vec!["dummy.do_something".to_string()],
                    work_done_progress_options: Default::default(),
                }),
                */
                // Configuramos para que podamos funcionar con workspaces
                /* 
                workspace: Some(WorkspaceServerCapabilities {
                    workspace_folders: Some(WorkspaceFoldersServerCapabilities {
                        supported: Some(true),
                        change_notifications: Some(OneOf::Left(true)),
                    }),
                    file_operations: None,
                }),
                */
                // Configuramos los colorcitos de los tokens
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensRegistrationOptions(
                        SemanticTokensRegistrationOptions {
                            text_document_registration_options: {
                                TextDocumentRegistrationOptions {
                                    // Queremos que funcione en los archivos terminados en "language"?
                                    document_selector: Some(vec![DocumentFilter {
                                        language: Some("nrs".to_string()),
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
                // definition: Some(GotoCapability::default()),
                //definition_provider: Some(OneOf::Left(true)),
                //references_provider: Some(OneOf::Left(true)),
                //rename_provider: Some(OneOf::Left(true)),
                ..ServerCapabilities::default()
            },
        })
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

    async fn execute_command(&self, _: ExecuteCommandParams) -> Result<Option<Value>> {
        self.client
            .log_message(MessageType::INFO, "command executed!")
            .await;

        match self.client.apply_edit(WorkspaceEdit::default()).await {
            Ok(res) if res.applied => self.client.log_message(MessageType::INFO, "applied").await,
            Ok(_) => self.client.log_message(MessageType::INFO, "rejected").await,
            Err(err) => self.client.log_message(MessageType::ERROR, err).await,
        }

        Ok(None)
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
    async fn inlay_hint(&self, params: InlayHintParams) -> Result<Vec<(usize, usize, String)>> {
        let mut hashmap = HashMap::new();
        // TODO update this
        if let Some(entry) = self.ast_map.get(&params.path) {
            let ast = &entry.0;
            let type_table = &entry.1;
            ast.iter().for_each(|(node, _, _)| {
                if let Ast::Binary(l, ":=", _) = node {
                    if let (Ast::Variable(_), span, t) = l.as_ref() {
                        let new_t = match t.clone().unwrap() {
                            Type::T(n) => type_table[n].clone(),
                            x => x,
                        };
                        hashmap.insert(span.clone(), new_t);
                    }
                }
            });
        }

        let inlay_hint_list = hashmap
            .into_iter()
            .map(|(k, v)| (k.start, k.end, format!("{:?}", v)))
            .collect::<Vec<_>>();
        Ok(inlay_hint_list)
    }
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

fn offset_to_position(offset: usize, rope: &Rope) -> Option<Position> {
    let line = rope.try_char_to_line(offset).ok()?;
    let first_char = rope.try_line_to_char(line).ok()?;
    let column = offset - first_char;
    Some(Position::new(line as u32, column as u32))
}
