# S2C MAROC — Android client

The phone app for the same ledger the web app serves. It talks only to this project's
own JSON API (`/api/v1`, documented in [`../docs/API.md`](../docs/API.md)) with a Bearer
token — no third-party SDK, no WebView, no bundled copy of the server.

Current version: **0.1.0** — first slice: sign in, dashboard, and every list.

| Sign in | Dashboard | A list |
| --- | --- | --- |
| ![Sign in](screens/login.png) | ![Dashboard](screens/dashboard.png) | ![Products](screens/products.png) |

## What works today

| Screen | What it shows |
| --- | --- |
| **Sign in** | Server address, username, password; the token and company are kept on the phone |
| **Dashboard** | Supplier/customer/product counts, low stock, stock value, receivables, payables, open POs, receivable aging |
| **Lists** | Suppliers, purchase orders, supplier invoices, payments, customers, quotes, customer orders, customer invoices, products |
| **Session** | Back returns to the dashboard; sign out clears the token |

Reading only: the app never writes to the ledger yet. Creating a supplier, issuing an
invoice or printing a ticket stays in the web app for now — see *Not built yet*.

## Build

From the repository root:

```bash
bash tools/build_android.sh          # → android/dist/s2c-maroc-0.1.0-debug.apk
```

The script uses the JDK bundled with Android Studio (`C:/Program Files/Android/Android
Studio/jbr`) because a machine whose `PATH` finds a JRE 1.8 cannot compile. Override it
with `S2C_JAVA_HOME=/path/to/jdk` if the SDK lives elsewhere.

Building directly:

```bash
export JAVA_HOME="C:/Program Files/Android/Android Studio/jbr"
gradle -p android assembleDebug      # org.gradle.java.home is also pinned in android/gradle.properties
```

`android/local.properties` points at the SDK (`sdk.dir`); it is machine-local and
git-ignored, as is `android/dist/`.

## Install

```bash
adb install -r android/dist/s2c-maroc-0.1.0-debug.apk
```

Or copy the APK to the phone and open it (allow "install unknown apps" for the file
manager). The APK is signed with the debug key: it is meant for the phone on your desk,
not for a store.

## Point it at your server

The first field on the sign-in screen is the server address, and it is remembered.

- **Emulator**: `http://10.0.2.2:8080` (the default) is the host machine's loopback.
- **Phone on the same Wi-Fi**: the PC's LAN address, e.g. `http://192.168.1.20:8080`.
  The server already binds `0.0.0.0`, so nothing needs changing on the PC; if the
  connection is refused, check Windows Firewall allows port 8080 on the private network.

Cleartext HTTP is allowed deliberately (`android:usesCleartextTraffic="true"`), because
an office server on the LAN has no certificate. Put the server behind TLS before the app
leaves a trusted network.

## Layout

```
android/
  settings.gradle.kts, build.gradle.kts, gradle.properties
  app/build.gradle.kts                  compileSdk 35, minSdk 24, versionName 0.1.0
  app/src/main/AndroidManifest.xml      INTERNET, cleartext, one activity
  app/src/main/res/                     theme, strings, adaptive launcher icon
  app/src/main/java/ma/s2c/maroc/
    MainActivity.java                   screens: sign in, dashboard, list, chrome
    Api.java                            HttpURLConnection + org.json, token in SharedPreferences
    Ui.java                             palette and view factories (Arabic above Latin)
```

Screens are built in Java rather than XML layouts because every label here is bilingual
and stacked — Arabic above the Latin — and that rule is easier to keep in one place than
across a pile of layout files.

## Not built yet

In the order they would matter on a shop floor:

1. **Writing**: create/edit suppliers, customers and products — the API already accepts
   `POST /suppliers`, `/customers`, `/products`, `/tickets`.
2. **Documents**: a purchase order, quote or invoice screen, and the actions on them
   (issue, validate, receive). Those endpoints are `GET` only today, so the API needs the
   writes first.
3. **Bluetooth receipts**: the browser already prints ESC/POS over Web Bluetooth; the
   native equivalent is `BluetoothLeScanner` + the same byte stream from
   `src/ticket.rs`.
4. **Offline**: nothing is cached; every screen needs the server.
5. **Release signing** with a key that belongs to the owner, and a version code scheme
   tied to the web release.
