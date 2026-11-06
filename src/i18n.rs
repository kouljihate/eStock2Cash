//! Multi-language support: English, French and Arabic (AR/EN and AR/FR).

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    En,
    Fr,
    Ar,
}

impl Lang {
    pub fn from_code(code: &str) -> Option<Self> {
        match code.trim().to_ascii_lowercase().as_str() {
            "en" => Some(Lang::En),
            "fr" => Some(Lang::Fr),
            "ar" => Some(Lang::Ar),
            _ => None,
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Fr => "fr",
            Lang::Ar => "ar",
        }
    }

    pub fn dir(&self) -> &'static str {
        match self {
            Lang::Ar => "rtl",
            _ => "ltr",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::Fr => "Français",
            Lang::Ar => "العربية",
        }
    }

    pub fn all() -> [Lang; 3] {
        [Lang::En, Lang::Fr, Lang::Ar]
    }
}

impl fmt::Display for Lang {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

macro_rules! lang_table {
    ($($key:literal => ($en:literal, $fr:literal, $ar:literal)),* $(,)?) => {
        pub fn t(lang: Lang, key: &str) -> &'static str {
            match (lang, key) {
                $((Lang::En, $key) => $en, (Lang::Fr, $key) => $fr, (Lang::Ar, $key) => $ar,)*
                _ => "",
            }
        }

        #[allow(dead_code)]
        pub const ALL_KEYS: &[&str] = &[$($key),*];
    };
}

lang_table! {
    "app_name" => ("S2C MAROC", "S2C MAROC", "S2C MAROC"),
    "app_tagline" => ("Procure-to-Pay · Quote-to-Cash", "Procure-to-Pay · Quote-to-Cash", "دورة الشراء · دورة البيع"),
    "lbl_languages" => ("Languages", "Langues", "اللغات"),
    "nav_dashboard" => ("Dashboard", "Tableau de bord", "لوحة التحكم"),
    "nav_purchasing" => ("Procure-to-Pay", "Procure-to-Pay", "دورة الشراء"),
    "nav_sales" => ("Quote-to-Cash", "Quote-to-Cash", "دورة البيع"),
    "nav_suppliers" => ("Suppliers", "Fournisseurs", "الموردون"),
    "nav_purchase_orders" => ("Purchase Orders", "Bons de commande", "أوامر الشراء"),
    "nav_goods_receipts" => ("Goods Receipts", "Réceptions", "استلام البضائع"),
    "nav_supplier_invoices" => ("Supplier Invoices", "Factures fournisseurs", "فواتير الموردين"),
    "nav_payments" => ("Payments", "Paiements", "المدفوعات"),
    "nav_customers" => ("Customers", "Clients", "العملاء"),
    "nav_quotes" => ("Quotes", "Devis", "عروض الأسعار"),
    "nav_customer_orders" => ("Customer Orders", "Commandes clients", "طلبات العملاء"),
    "nav_delivery_notes" => ("Delivery Notes", "Bons de livraison", "إشعارات التسليم"),
    "nav_customer_invoices" => ("Customer Invoices", "Factures clients", "فواتير العملاء"),
    "nav_products" => ("Products", "Produits", "المنتجات"),
    "nav_credit" => ("Credit", "Crédit", "الائتمان"),
    "nav_tickets" => ("Direct Tickets", "Tickets directs", "تذاكر البيع المباشر"),
    "nav_admin" => ("Administration", "Administration", "الإدارة"),
    "nav_events" => ("Event Log", "Journal d'événements", "سجل الأحداث"),
    "nav_broker" => ("Message Broker", "Broker de messages", "وسيط الرسائل"),

    "lbl_name" => ("Name", "Nom", "الاسم"),
    "lbl_code" => ("Code", "Code", "الرمز"),
    "lbl_legal_id" => ("Legal ID", "Identifiant légal", "المعرف القانوني"),
    "lbl_tax_id" => ("Tax ID", "Numéro fiscal", "الرقم الضريبي"),
    "lbl_email" => ("Email", "Email", "البريد الإلكتروني"),
    "lbl_phone" => ("Phone", "Téléphone", "الهاتف"),
    "lbl_address" => ("Address", "Adresse", "العنوان"),
    "lbl_city" => ("City", "Ville", "المدينة"),
    "lbl_country" => ("Country", "Pays", "البلد"),
    "lbl_contact" => ("Contact person", "Personne de contact", "مسؤول التواصل"),
    "lbl_notes" => ("Notes", "Notes", "ملاحظات"),
    "lbl_status" => ("Status", "Statut", "الحالة"),
    "lbl_active" => ("Active", "Actif", "نشط"),
    "lbl_inactive" => ("Inactive", "Inactif", "غير نشط"),
    "lbl_sku" => ("SKU", "Référence", "رمز المنتج"),
    "lbl_unit" => ("Unit", "Unité", "الوحدة"),
    "lbl_category" => ("Category", "Catégorie", "الفئة"),
    "lbl_cost" => ("Cost price", "Prix d'achat", "سعر التكلفة"),
    "lbl_price" => ("Sale price", "Prix de vente", "سعر البيع"),
    "lbl_stock" => ("In stock", "En stock", "المخزون"),
    "lbl_reorder" => ("Reorder level", "Seuil de réappro.", "حد إعادة الطلب"),
    "lbl_quantity" => ("Qty", "Qté", "الكمية"),
    "lbl_unit_price" => ("Unit price", "Prix unitaire", "سعر الوحدة"),
    "lbl_tax_rate" => ("Tax %", "TVA %", "الضريبة %"),
    "lbl_date" => ("Date", "Date", "التاريخ"),
    "lbl_due_date" => ("Due date", "Échéance", "تاريخ الاستحقاق"),
    "lbl_number" => ("Number", "Numéro", "الرقم"),
    "lbl_total" => ("Total", "Total", "الإجمالي"),
    "lbl_subtotal" => ("Subtotal", "Sous-total", "المجموع الفرعي"),
    "lbl_tax" => ("Tax", "TVA", "الضريبة"),
    "lbl_actions" => ("Actions", "Actions", "إجراءات"),
    "lbl_supplier" => ("Supplier", "Fournisseur", "المورد"),
    "lbl_customer" => ("Customer", "Client", "العميل"),
    "lbl_product" => ("Product", "Produit", "المنتج"),
    "lbl_search" => ("Search", "Rechercher", "بحث"),
    "lbl_credit_limit" => ("Credit limit", "Plafond de crédit", "حد الائتمان"),
    "lbl_balance" => ("Balance", "Solde", "الرصيد"),
    "lbl_exposure" => ("Outstanding", "Encours", "المستحق"),
    "lbl_reference" => ("Reference", "Référence", "المرجع"),
    "lbl_method" => ("Method", "Moyen", "طريقة الدفع"),
    "lbl_amount" => ("Amount", "Montant", "المبلغ"),
    "lbl_paid" => ("Paid", "Payé", "المدفوع"),
    "lbl_change" => ("Change", "Rendu", "الباقي"),
    "lbl_cashier" => ("Cashier", "Caissier", "أمين الصندوق"),
    "lbl_language" => ("Language", "Langue", "اللغة"),
    "lbl_username" => ("Username", "Identifiant", "اسم المستخدم"),
    "lbl_password" => ("Password", "Mot de passe", "كلمة المرور"),
    "lbl_current_password" => ("Current password", "Mot de passe actuel", "كلمة المرور الحالية"),
    "lbl_new_password" => ("New password", "Nouveau mot de passe", "كلمة المرور الجديدة"),
    "lbl_full_name" => ("Full name", "Nom complet", "الاسم الكامل"),
    "lbl_company" => ("Company", "Société", "الشركة"),
    "lbl_currency" => ("Currency", "Devise", "العملة"),
    "lbl_header" => ("Header text", "Texte d'en-tête", "نص الترويسة"),
    "lbl_footer" => ("Footer text", "Texte de pied", "نص التذييل"),
    "lbl_logo" => ("Logo (JPEG)", "Logo (JPEG)", "الشعار (JPEG)"),
    "lbl_primary_color" => ("Primary colour", "Couleur principale", "اللون الأساسي"),
    "lbl_accent_color" => ("Accent colour", "Couleur d'accent", "اللون الثانوي"),
    "lbl_font" => ("Font family", "Police", "الخط"),
    "lbl_valid_until" => ("Valid until", "Valable jusqu'au", "صالح حتى"),
    "lbl_expected_date" => ("Expected date", "Date prévue", "التاريخ المتوقع"),
    "lbl_order_date" => ("Order date", "Date de commande", "تاريخ الطلب"),
    "lbl_invoice_date" => ("Invoice date", "Date de facture", "تاريخ الفاتورة"),
    "lbl_receipt_date" => ("Receipt date", "Date de réception", "تاريخ الاستلام"),
    "lbl_delivery_date" => ("Delivery date", "Date de livraison", "تاريخ التسليم"),
    "lbl_description" => ("Description", "Description", "الوصف"),
    "lbl_lines" => ("Lines", "Lignes", "البنود"),
    "lbl_document" => ("Document", "Document", "المستند"),
    "lbl_source" => ("Source", "Origine", "المصدر"),
    "lbl_created_by" => ("Created by", "Créé par", "أنشأه"),
    "lbl_created_at" => ("Created", "Créé le", "تاريخ الإنشاء"),
    "lbl_outstanding" => ("Outstanding", "Reste dû", "المتبقي"),
    "lbl_days" => ("Days", "Jours", "أيام"),
    "lbl_qty_ordered" => ("Ordered", "Commandé", "المطلوب"),
    "lbl_qty_received" => ("Received", "Reçu", "المستلم"),
    "lbl_received" => ("Received", "Reçu", "تم الاستلام"),
    "lbl_billed" => ("Billed", "Facturé", "المفوتر"),
    "lbl_none" => ("None", "Aucun", "لا يوجد"),
    "lbl_role" => ("Role", "Rôle", "الدور"),

    "a_new" => ("New", "Nouveau", "جديد"),
    "a_save" => ("Save", "Enregistrer", "حفظ"),
    "a_cancel" => ("Cancel", "Annuler", "إلغاء"),
    "a_back" => ("Back", "Retour", "رجوع"),
    "a_edit" => ("Edit", "Modifier", "تعديل"),
    "a_delete" => ("Delete", "Supprimer", "حذف"),
    "a_view" => ("Open", "Ouvrir", "فتح"),
    "a_search" => ("Search", "Rechercher", "بحث"),
    "a_clear" => ("Clear", "Effacer", "مسح"),
    "a_add_line" => ("Add line", "Ajouter une ligne", "إضافة بند"),
    "a_remove" => ("Remove", "Retirer", "إزالة"),
    "a_send" => ("Send", "Envoyer", "إرسال"),
    "a_post" => ("Post", "Valider", "ترحيل"),
    "a_issue" => ("Issue", "Émettre", "إصدار"),
    "a_accept" => ("Validate", "Valider", "اعتماد"),
    "a_convert" => ("Convert", "Convertir", "تحويل"),
    "a_deliver" => ("Deliver", "Livrer", "تسليم"),
    "a_invoice" => ("Invoice", "Facturer", "فوترة"),
    "a_print" => ("Print", "Imprimer", "طباعة"),
    "a_pdf" => ("PDF", "PDF", "PDF"),
    "a_pay" => ("Record payment", "Enregistrer un paiement", "تسجيل دفعة"),
    "a_login" => ("Sign in", "Se connecter", "تسجيل الدخول"),
    "a_logout" => ("Sign out", "Déconnexion", "تسجيل الخروج"),
    "a_change_password" => ("Change password", "Changer le mot de passe", "تغيير كلمة المرور"),
    "a_adjust" => ("Adjust stock", "Ajuster le stock", "تعديل المخزون"),
    "a_add" => ("Add", "Ajouter", "إضافة"),
    "a_apply" => ("Apply", "Appliquer", "تطبيق"),
    "a_bluetooth" => ("Bluetooth print", "Imprimer en Bluetooth", "طباعة بلوتوث"),

    "st_draft" => ("Draft", "Brouillon", "مسودة"),
    "st_sent" => ("Sent", "Envoyé", "مُرسل"),
    "st_confirmed" => ("Confirmed", "Confirmé", "مؤكد"),
    "st_partially_received" => ("Partially received", "Partiellement reçu", "مستلم جزئياً"),
    "st_received" => ("Received", "Reçu", "تم الاستلام"),
    "st_delivered" => ("Delivered", "Livré", "تم التسليم"),
    "st_invoiced" => ("Invoiced", "Facturé", "مفوتر"),
    "st_accepted" => ("Accepted", "Accepté", "مقبول"),
    "st_rejected" => ("Rejected", "Refusé", "مرفوض"),
    "st_expired" => ("Expired", "Expiré", "منتهي"),
    "st_converted" => ("Converted", "Converti", "محوّل"),
    "st_open" => ("Open", "Ouverte", "مفتوحة"),
    "st_partially_paid" => ("Partially paid", "Partiellement payée", "مدفوعة جزئياً"),
    "st_paid" => ("Paid", "Payée", "مدفوعة"),
    "st_overdue" => ("Overdue", "En retard", "متأخرة"),
    "st_posted" => ("Posted", "Validé", "مُرحّل"),
    "st_cancelled" => ("Cancelled", "Annulé", "ملغى"),

    "msg_saved" => ("Saved successfully.", "Enregistré avec succès.", "تم الحفظ بنجاح."),
    "msg_deleted" => ("Record deleted.", "Enregistrement supprimé.", "تم حذف السجل."),
    "msg_posted" => ("Document posted and stock updated.", "Document validé et stock mis à jour.", "تم ترحيل المستند وتحديث المخزون."),
    "msg_issued" => ("Document issued.", "Document émis.", "تم إصدار المستند."),
    "msg_sent" => ("Document marked as sent.", "Document marqué comme envoyé.", "تم وضع المستند كمُرسل."),
    "msg_invalid_credentials" => ("Invalid username or password.", "Identifiant ou mot de passe invalide.", "اسم المستخدم أو كلمة المرور غير صحيحة."),
    "msg_not_found" => ("Record not found.", "Enregistrement introuvable.", "السجل غير موجود."),
    "msg_no_records" => ("No records yet.", "Aucun enregistrement.", "لا توجد سجلات بعد."),
    "msg_required" => ("Please fill in all required fields.", "Veuillez remplir les champs obligatoires.", "يرجى ملء الحقول المطلوبة."),
    "msg_credit_exceeded" => ("Credit limit exceeded for this customer.", "Plafond de crédit dépassé pour ce client.", "تم تجاوز حد الائتمان لهذا العميل."),
    "msg_invalid_amount" => ("Invalid amount.", "Montant invalide.", "مبلغ غير صالح."),
    "msg_amount_too_high" => ("Amount exceeds the outstanding balance.", "Le montant dépasse le reste dû.", "المبلغ يتجاوز المتبقي."),
    "msg_already_converted" => ("This quote has already been converted.", "Ce devis a déjà été converti.", "تم تحويل هذا العرض مسبقاً."),
    "msg_confirm_delete" => ("Delete this record?", "Supprimer cet enregistrement ?", "حذف هذا السجل؟"),
    "msg_welcome" => ("Welcome back", "Bon retour", "مرحباً بعودتك"),
    "msg_logged_out" => ("You have been signed out.", "Vous êtes déconnecté.", "تم تسجيل خروجك."),
    "msg_password_changed" => ("Password updated.", "Mot de passe mis à jour.", "تم تحديث كلمة المرور."),
    "msg_stock_adjusted" => ("Stock adjusted.", "Stock ajusté.", "تم تعديل المخزون."),
    "msg_quote_converted" => ("Quote validated: sales order and invoice created.", "Devis validé : commande et facture créées.", "تم اعتماد العرض: إنشاء الطلب والفاتورة."),
    "msg_cannot_post_twice" => ("This document is already posted.", "Ce document est déjà validé.", "تم ترحيل هذا المستند مسبقاً."),
    "msg_line_required" => ("Add at least one line with a description.", "Ajoutez au moins une ligne.", "أضف بنداً واحداً على الأقل."),
    "msg_error" => ("Something went wrong.", "Une erreur est survenue.", "حدث خطأ ما."),
    "msg_line_removed" => ("Line removed.", "Ligne supprimée.", "تم حذف البند."),
    "msg_duplicate" => ("A record with this code/number already exists.", "Un enregistrement avec ce code existe déjà.", "يوجد سجل بهذا الرمز مسبقاً."),

    "m_low_stock" => ("Low stock", "Stock faible", "مخزون منخفض"),
    "m_stock_value" => ("Stock value", "Valeur du stock", "قيمة المخزون"),
    "m_receivables" => ("Receivables", "Créances", "المستحقات"),
    "m_payables" => ("Payables", "Dettes", "الالتزامات"),
    "m_open_documents" => ("Open documents", "Documents ouverts", "مستندات مفتوحة"),
    "m_quick_actions" => ("Quick actions", "Actions rapides", "إجراءات سريعة"),
    "m_recent" => ("Recent activity", "Activité récente", "النشاط الأخير"),
    "m_company_profile" => ("Company profile", "Profil de la société", "ملف الشركة"),
    "m_invoice_template" => ("Invoice template", "Modèle de facture", "قالب الفاتورة"),
    "m_users" => ("Users", "Utilisateurs", "المستخدمون"),
    "m_event_log" => ("Event log", "Journal d'événements", "سجل الأحداث"),
    "m_rabbitmq" => ("RabbitMQ", "RabbitMQ", "RabbitMQ"),
    "m_management_api" => ("Management API", "API de gestion", "واجهة الإدارة"),
    "m_queues" => ("Queues", "Files", "قوائم الانتظار"),
    "m_messages" => ("Messages", "Messages", "الرسائل"),
    "m_connected" => ("Connected", "Connecté", "متصل"),
    "m_disconnected" => ("Disconnected", "Déconnecté", "غير متصل"),
    "m_not_configured" => ("Not configured", "Non configuré", "غير مُهيّأ"),
    "m_broker_offline" => ("Management API unreachable - the app keeps working; events stay unpublished until RabbitMQ is back.", "API de gestion injoignable - l'application continue de fonctionner.", "واجهة الإدارة غير متاحة - يستمر التطبيق في العمل."),
    "m_direct_purchase" => ("Direct purchase", "Achat direct", "بيع مباشر"),
    "m_sample_admin" => ("Default local administrator account", "Compte administrateur local par défaut", "حساب المدير المحلي الافتراضي"),
    "m_fonts_hint" => ("Drop Comfortaa / VIP RAWY font files into static/fonts to enable the branded typefaces.", "Placez les polices Comfortaa / VIP RAWY dans static/fonts.", "ضع خطوط Comfortaa / VIP RAWY في مجلد static/fonts."),
    "m_no_broker" => ("RABBITMQ_URL is not set, messaging is disabled.", "RABBITMQ_URL non défini, messagerie désactivée.", "لم يتم ضبط RABBITMQ_URL، المراسلة معطلة."),
    "m_signed_in_as" => ("Signed in as", "Connecté en tant que", "مسجّل الدخول باسم"),
    "m_aging" => ("Aging", "Balance âgée", "أعمار الديون"),
    "m_current" => ("Current", "Non échu", "غير مستحق"),
    "m_credit_usage" => ("Credit usage", "Utilisation du crédit", "استخدام الائتمان"),
    "m_unlimited" => ("Unlimited", "Illimité", "غير محدود"),
    "m_ticket_printer" => ("Mini printer", "Mini imprimante", "طابعة صغيرة"),
    "m_ticket_hint" => ("Use the raw ESC/POS endpoint with a USB/serial printer, or pair a Bluetooth printer.", "Utilisez l'endpoint ESC/POS avec une imprimante USB/série, ou une imprimante Bluetooth.", "استخدم نقطة ESC/POS مع طابعة USB/تسلسلية أو طابعة بلوتوث."),
    "m_totals" => ("Totals", "Totaux", "المجاميع"),
    "m_document_chain" => ("Document chain", "Chaîne documentaire", "سلسلة المستندات"),
    "m_days_old" => ("days", "jours", "يوم"),
    "m_no_lines" => ("No lines on this document.", "Aucune ligne sur ce document.", "لا توجد بنود في هذا المستند."),
    "m_select_product" => ("Select a product", "Choisir un produit", "اختر منتجاً"),
    "m_free_text" => ("Free-text line", "Ligne libre", "بند حر"),
    "m_print_section" => ("Printing", "Impression", "الطباعة"),
}

/// Translate `key`; falls back to the key itself so a missing label is obvious.
pub fn tr(lang: Lang, key: &str) -> String {
    let s = t(lang, key);
    if s.is_empty() {
        key.to_string()
    } else {
        s.to_string()
    }
}
