/* S2C MAROC — progressive enhancement only; every form works without JS. */
(function () {
  "use strict";

  var money = function (n) {
    return (Math.round(n * 100) / 100).toFixed(2);
  };

  function mutate(fn) { fn(); }

  /* ---------------- document line editor ---------------- */
  function recalc(wrap) {
    var sumSub = document.getElementById("sum-sub");
    var sumTax = document.getElementById("sum-tax");
    var sumTotal = document.getElementById("sum-total");
    var sub = 0, tax = 0;

    wrap.querySelectorAll("tr.line-row").forEach(function (row) {
      var q = parseFloat((row.querySelector('input[name="item_qty"]') || {}).value) || 0;
      var p = parseFloat((row.querySelector('input[name="item_price"]') || {}).value) || 0;
      var t = parseFloat((row.querySelector('input[name="item_tax"]') || {}).value) || 0;
      var net = q * p;
      var lineTax = (net * t) / 100;
      sub += net;
      tax += lineTax;
      var cell = row.querySelector(".line-total");
      if (cell) cell.textContent = money(net);
    });

    if (sumSub) sumSub.textContent = money(sub);
    if (sumTax) sumTax.textContent = money(tax);
    if (sumTotal) sumTotal.textContent = money(sub + tax);
  }

  function initLines() {
    var wrap = document.querySelector(".lines-wrap");
    if (!wrap) return;
    var body = wrap.querySelector("#lines-body");
    var tpl = wrap.querySelector("#line-tpl");
    var addBtn = wrap.querySelector("#add-line");
    var mode = wrap.getAttribute("data-mode") || "sale";

    if (addBtn && tpl && body) {
      addBtn.addEventListener("click", function () {
        body.appendChild(tpl.content.cloneNode(true));
        recalc(wrap);
      });
    }

    wrap.addEventListener("click", function (e) {
      if (e.target.classList.contains("remove-line")) {
        var row = e.target.closest("tr.line-row");
        if (row && body && body.querySelectorAll("tr.line-row").length > 1) {
          row.remove();
          recalc(wrap);
        }
      }
    });

    wrap.addEventListener("change", function (e) {
      var row = e.target.closest("tr.line-row");
      if (!row) return;
      if (e.target.classList.contains("line-product")) {
        var opt = e.target.options[e.target.selectedIndex];
        if (!opt) return;
        var price = opt.getAttribute(mode === "cost" ? "data-cost" : "data-price");
        var priceInput = row.querySelector('input[name="item_price"]');
        var descInput = row.querySelector('input[name="item_desc"]');
        if (price && priceInput && (parseFloat(priceInput.value) || 0) === 0) {
          priceInput.value = price;
        }
        // the option may carry the bare name when its label also shows price and
        // stock, as the till does
        var name =
          opt.getAttribute("data-name") ||
          (opt.textContent || "").replace(/\s*\(.*\)\s*$/, "").trim();
        if (descInput && !descInput.value && name) {
          descInput.value = name;
        }
      }
      recalc(wrap);
    });

    wrap.addEventListener("input", function (e) {
      if (e.target.name === "item_qty" || e.target.name === "item_price" || e.target.name === "item_tax") {
        recalc(wrap);
      }
    });

    recalc(wrap);
  }

  /* ---------------- cash registry ---------------- */
  // The till reads the running total the line editor keeps in #sum-total, so the
  // change is right whatever the cashier types.
  function initCash() {
    var paid = document.getElementById("cash-received");
    var shown = document.getElementById("cash-change");
    var totalEl = document.getElementById("sum-total");
    if (!paid || !shown || !totalEl) return;

    var update = function () {
      var total = parseFloat(totalEl.textContent) || 0;
      var given = parseFloat(paid.value) || 0;
      shown.textContent = money(given - total);
      shown.classList.toggle("short", given + 0.005 < total);
    };

    document.querySelectorAll("[data-tender]").forEach(function (btn) {
      btn.addEventListener("click", function () {
        paid.value = btn.getAttribute("data-tender");
        update();
      });
    });
    var exact = document.querySelector("[data-tender-exact]");
    if (exact) {
      exact.addEventListener("click", function () {
        paid.value = money(parseFloat(totalEl.textContent) || 0);
        update();
      });
    }

    // The keypad writes the amount received, one tap at a time.
    document.querySelectorAll(".keypad [data-key]").forEach(function (btn) {
      btn.addEventListener("click", function () {
        var k = btn.getAttribute("data-key");
        var v = paid.value || "";
        if (k === "clear") {
          v = "";
        } else if (k === "back") {
          v = v.slice(0, -1);
        } else {
          if (k === "." && v.indexOf(".") !== -1) return;
          v = (v === "" && k === "." ? "0" : v) + k;
        }
        paid.value = v;
        update();
      });
    });

    document.addEventListener("input", update);
    document.addEventListener("change", update);
    update();
  }

  // The till takes cash today or lets the customer pay on payment day: only the
  // block that matters is on screen.
  function initCashMode() {
    var mode = document.getElementById("cash-mode");
    if (!mode) return;

    var sync = function () {
      var credit = mode.value === "credit";
      document.querySelectorAll(".cash-only").forEach(function (el) {
        el.hidden = credit;
      });
      document.querySelectorAll(".credit-only").forEach(function (el) {
        el.hidden = !credit;
      });
      // a field nobody can see must not hold the form back
      var paid = document.getElementById("cash-received");
      if (paid) paid.disabled = credit;
    };

    mode.addEventListener("change", sync);
    sync();
  }

  /* ---------------- receipt straight from the till ---------------- */
  function initAutoprint() {
    if (!document.querySelector("[data-autoprint]")) return;
    window.addEventListener("load", function () {
      setTimeout(function () { window.print(); }, 250);
    });
  }

  /* ---------------- flash messages ---------------- */
  function initFlash() {
    document.querySelectorAll(".flash").forEach(function (el) {
      setTimeout(function () {
        el.style.transition = "opacity .4s";
        el.style.opacity = "0";
        setTimeout(function () { el.remove(); }, 400);
      }, 6000);
    });
  }

  /* ---------------- double submit guard ---------------- */
  function initForms() {
    document.querySelectorAll("form").forEach(function (form) {
      form.addEventListener("submit", function (e) {
        var btn = form.querySelector('button[type="submit"]');
        if (!btn) return;
        // the delete confirmation cancels the first submit, so only the one that
        // really posts may disable its button
        setTimeout(function () {
          if (!e.defaultPrevented) btn.disabled = true;
        }, 0);
      });
    });
  }

  /* ---------------- delete confirmation ---------------- */
  /* A form that carries `data-confirm` does not post straight away: it opens the
     <dialog> the shell drew, which names the record and asks in both languages. The
     answer comes back as the dialog's returnValue, and only then does the form post.
     Browsers without dialog support fall back to their own box. */
  function initConfirm() {
    var forms = document.querySelectorAll("form[data-confirm]");
    if (!forms.length) return;
    var dlg = document.getElementById("confirm-dialog");

    if (!dlg || typeof dlg.showModal !== "function") {
      forms.forEach(function (form) {
        form.addEventListener("submit", function (e) {
          var text = form.getAttribute("data-confirm-note") || form.getAttribute("data-confirm");
          if (!window.confirm(text)) e.preventDefault();
        });
      });
      return;
    }

    var pending = null;
    var subject = dlg.querySelector("[data-confirm-subject]");
    var yes = dlg.querySelector("[data-confirm-ok]");
    var no = dlg.querySelector("[data-confirm-cancel]");

    forms.forEach(function (form) {
      form.addEventListener("submit", function (e) {
        if (form.getAttribute("data-confirmed") === "1") return; // the second pass posts
        e.preventDefault();
        pending = form;
        var text = form.getAttribute("data-confirm-note") || form.getAttribute("data-confirm") || "";
        if (subject) {
          subject.textContent = text;
          subject.hidden = !text;
        }
        dlg.showModal();
        if (no) no.focus();
      });
    });

    /* The answer is read from the buttons rather than from the dialog's close event:
       some builds never deliver one, and a confirmation that silently does nothing is
       the worst outcome for a delete. Escape closes the dialog through the browser,
       which leaves `pending` alone — the next submit overwrites it. */
    if (no) {
      no.addEventListener("click", function () {
        pending = null;
        dlg.close();
      });
    }

    if (yes) {
      yes.addEventListener("click", function () {
        var form = pending;
        pending = null;
        dlg.close();
        if (!form) return;
        form.setAttribute("data-confirmed", "1");
        if (typeof form.requestSubmit === "function") {
          form.requestSubmit();
        } else {
          form.submit();
        }
      });
    }
  }

  /* ---------------- Bluetooth ESC/POS printing ---------------- */
  /* Mini printers expose the common ESC/POS service 0x18F0. */
  function initBluetooth() {
    var btn = document.querySelector(".bt-print");
    if (!btn) return;
    if (!navigator.bluetooth) {
      btn.addEventListener("click", function () {
        alert("Web Bluetooth is not available in this browser. Use the ESC/POS link with a USB/serial printer.");
      });
      return;
    }
    btn.addEventListener("click", async function () {
      var url = btn.getAttribute("data-url");
      try {
        btn.disabled = true;
        var device = await navigator.bluetooth.requestDevice({
          filters: [{ services: [0x18f0] }],
          optionalServices: [0x18f0, 0xff00, 0x1101]
        });
        var server = await device.gatt.connect();
        var service = await server.getPrimaryService(0x18f0);
        var characteristic = await service.getCharacteristic(0x2af1);
        var resp = await fetch(url, { credentials: "same-origin" });
        if (!resp.ok) throw new Error("could not fetch ticket data");
        var buffer = new Uint8Array(await resp.arrayBuffer());
        var chunk = 100;
        for (var i = 0; i < buffer.length; i += chunk) {
          await characteristic.writeValue(buffer.slice(i, i + chunk));
        }
        btn.textContent = "Printed";
      } catch (err) {
        console.error(err);
        alert("Bluetooth printing failed: " + err.message);
      } finally {
        setTimeout(function () { btn.disabled = false; }, 1500);
      }
    });
  }

  /* ---------------- collapsible sidebar ---------------- */
  function initSidebarToggle() {
    var btn = document.querySelector("[data-nav-toggle]");
    if (!btn) return;

    function sync() {
      var closed = document.documentElement.classList.contains("nav-closed");
      btn.setAttribute("aria-expanded", closed ? "false" : "true");
    }

    sync();
    btn.addEventListener("click", function () {
      var closed = document.documentElement.classList.toggle("nav-closed");
      try {
        localStorage.setItem("s2c-nav", closed ? "closed" : "open");
      } catch (e) {}
      sync();
    });
  }

  /* ---------------- collapsible menu groups ---------------- */
  /* <details> folds on its own; this only remembers the choice, and always opens
     the group that holds the current page so the active link is never hidden. */
  function initNavSections() {
    var secs = document.querySelectorAll("details.nav-section[data-nav-sec]");
    if (!secs.length) return;

    var KEY = "s2c-nav-secs";
    var folded = {};
    try {
      folded = JSON.parse(localStorage.getItem(KEY) || "{}") || {};
    } catch (e) {
      folded = {};
    }

    secs.forEach(function (sec) {
      var key = sec.getAttribute("data-nav-sec");
      sec.addEventListener("toggle", function () {
        folded[key] = !sec.open;
        try {
          localStorage.setItem(KEY, JSON.stringify(folded));
        } catch (e) {}
      });
      // restore the folded groups, but never hide the page you are on
      if (sec.querySelector(".nav-item.active")) {
        sec.open = true;
      } else if (folded[key]) {
        sec.open = false;
      }
    });
  }

  /* ---------------- header settings menu ---------------- */
  function closeSettings() {
    document.querySelectorAll("details.settings[open]").forEach(function (d) {
      d.removeAttribute("open");
      d.querySelectorAll("details.sub[open]").forEach(function (s) {
        s.removeAttribute("open");
      });
    });
  }

  function initSettingsMenu() {
    if (!document.querySelector("details.settings")) return;

    document.addEventListener("click", function (e) {
      document.querySelectorAll("details.settings[open]").forEach(function (d) {
        if (!d.contains(e.target)) d.removeAttribute("open");
      });
    });

    document.addEventListener("keydown", function (e) {
      if (e.key === "Escape") closeSettings();
    });
  }

  mutate(function () {
    document.addEventListener("DOMContentLoaded", function () {
      initLines();
      initFlash();
      initForms();
      initConfirm();
      initBluetooth();
      initSettingsMenu();
      initSidebarToggle();
      initNavSections();
      initCash();
      initCashMode();
      initAutoprint();
    });
  });
})();
