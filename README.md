# PteroHub
Client desktop indépendant pour Pterodactyl. Version 0.3.0.

Windows: installer Rust stable, Node.js 22, Visual Studio Build Tools (C++) et WebView2. Exécuter `npm install` puis `npm run tauri dev`. Pour un exécutable sans installateur: `npm run tauri build -- --no-bundle`. WebView2 reste requis.

Les clés sont saisies à chaque session et conservées uniquement en mémoire Rust. Révoquer toute clé déjà divulguée. Utiliser uniquement un panel autorisé, HTTPS et des permissions minimales. Les routes V2 personnalisées restent à vérifier.

La version 0.3.0 propose inventaire client/admin, ressources, contrôle de puissance et consultation des fichiers, sauvegardes, tâches, bases SQL, allocations et sous-utilisateurs. Les routes doivent être validées sur chaque panel. Console WebSocket, fichiers, CRUD admin, RPC et updater signé restent à intégrer/tester.
