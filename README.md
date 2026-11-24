# PteroHub
Client desktop indépendant pour Pterodactyl. Version initiale 0.1.0.

Windows: installer Rust stable, Node.js 22, Visual Studio Build Tools (C++) et WebView2. Exécuter `npm install` puis `npm run tauri dev`. Pour un exécutable sans installateur: `npm run tauri build -- --no-bundle`. WebView2 reste requis.

Les clés sont saisies à chaque session et conservées uniquement en mémoire Rust. Révoquer toute clé déjà divulguée. Utiliser uniquement un panel autorisé, HTTPS et des permissions minimales. Les routes V2 personnalisées restent à vérifier.

La version initiale propose inventaire client/admin, ressources et contrôle de puissance. Console WebSocket, fichiers, CRUD admin, RPC et updater signé restent à intégrer/tester.
