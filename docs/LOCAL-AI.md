# Optional local question answering

For TablesCraftLocal 0.9.6-local.20261103. A compatible local OpenAI-style runtime and downloaded chat model must already be installed separately; neither is bundled.

1. Run the server on literal loopback `127.0.0.1`, typically port 1234. Do not expose it to the network. Choose the model you have loaded; no model download or current model recommendation is performed by this app.
2. Open the Settings gear, enable the connection for this session, list local models and select the loaded model. Optional API authentication stays in memory.
3. Choose a supported context scope and write the question. Press Preview and inspect the exact question/excerpts. Press Send to local AI only when you intend to send them. Selection/context menus capture a snapshot and do not send by themselves.
4. Read or copy the answer. Generated content is inert and does not automatically edit documents. Cancel requests if needed; the server may take longer to finish its own inference.

Only up to eight excerpts and 12,000 UTF-8 text bytes are selected. Byte limits are not token counts. Lexical selection can miss relevant information. A whole-document question does not guarantee whole-document coverage. Context, hidden content, copy permissions and supported selections differ by application. Word body extraction excludes secondary stories; Tables excludes hidden sheets/rows/columns and hidden protected formulas; Deck excludes hidden slides/shapes and does not interpret image pixels; PDF needs readable text/OCR and respects copy permission, with a bounded page scope.

The desktop client accepts only the literal loopback address and selected numeric port, does not follow redirects or inherit proxy settings, and does not download/execute models or generated actions. Model quality and availability depend on the separate runtime. Packaging did not send any live model request.
