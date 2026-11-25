use crate::panel::*;

pub(crate) const CSS: &str = "body{font:15px system-ui,sans-serif;background:#f4f4f5;color:#27272a;margin:0}\
main{max-width:780px;margin:0 auto;padding:16px}nav{background:#fff;border-bottom:1px solid #e4e4e7;padding:10px 16px;display:flex;gap:16px;align-items:center}\
nav a{color:#52525b;text-decoration:none}nav a:hover{color:#18181b}nav form{margin-left:auto}h1{font-size:20px}\
.card{background:#fff;border:1px solid #e4e4e7;border-radius:8px;padding:12px 14px;margin:8px 0}\
.row{display:flex;gap:10px;align-items:center;justify-content:space-between;flex-wrap:wrap}\
button{background:#3f3f46;color:#fff;border:0;border-radius:6px;padding:6px 12px;cursor:pointer}button:hover{background:#18181b}\
button.off{background:#e4e4e7;color:#27272a}input,select{padding:6px;border:1px solid #d4d4d8;border-radius:6px}\
code{color:#71717a;font-size:12px}.tag{background:#e4e4e7;border-radius:10px;padding:2px 8px;font-size:12px}\
.tag.on{background:#3f3f46;color:#fff}table{width:100%;border-collapse:collapse}td,th{text-align:left;padding:4px;border-bottom:1px solid #e4e4e7}\
.code{font:700 32px monospace;letter-spacing:4px}";

pub(crate) fn page(title: &str, body: &str) -> String {
    format!(
        "<!doctype html><html lang=\"pt-BR\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
         <title>Silicon - {t}</title><style>{CSS}</style></head><body>\
         <nav><b>Silicon</b><a href=\"/\">Status</a><a href=\"/chats\">Chats</a><a href=\"/contacts\">Contatos</a><a href=\"/rules\">Regras</a><a href=\"/alerts\">Alertas</a><a href=\"/debug\">Debug</a><a href=\"/pair\">Pareamento</a><a href=\"/downloader\">Downloader</a><a href=\"/settings\">Ajustes</a>\
         <form method=\"post\" action=\"/logout\"><button class=\"off\">Sair</button></form></nav>\
         <main><h1>{t}</h1>{body}</main></body></html>",
        t = esc(title)
    )
}

pub(crate) fn login_page(msg: &str) -> String {
    format!(
        "<!doctype html><html lang=\"pt-BR\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
         <title>Silicon - entrar</title><style>{CSS}</style></head><body><main><h1>Silicon</h1><p>{}</p>\
         <form class=\"card\" method=\"post\" action=\"/login\">\
         <input type=\"password\" name=\"password\" autofocus> <button>Entrar</button></form></main></body></html>",
        esc(msg)
    )
}
