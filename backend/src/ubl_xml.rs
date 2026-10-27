//! UBL 2.1 / Peppol BIS → полета за чернова. Без база.

pub struct Party {
    pub name: String,
    pub eik: String,
    pub vat: String,
    pub country: String,
}

pub struct Line {
    pub qty: String,
    pub unit: String,
    pub net: String,
    pub name: String,
    pub seller: String,
    pub price: String,
    pub pct: String,
}

pub struct Draft {
    pub root: String,
    pub number: String,
    pub issue_date: String,
    pub due_date: String,
    pub tax_point: String,
    pub type_code: String,
    pub currency: String,
    pub currency_rate: String,
    pub pay_code: String,
    pub note: String,
    pub net: String,
    pub vat: String,
    pub total: String,
    pub tax_cat: String,
    pub exemption: String,
    pub original: String,
    pub supplier: Party,
    pub customer: Party,
    pub lines: Vec<Line>,
}

struct Open {
    name: String,
    unit: String,
}

fn local(tag: &str) -> String {
    let head = tag.split_whitespace().next().unwrap_or(tag);
    head.rsplit(':').next().unwrap_or(head).to_string()
}

fn attr(tag: &str, key: &str) -> String {
    let needle = format!("{key}=\"");
    let Some(pos) = tag.find(&needle) else {
        return String::new();
    };
    let rest = &tag[pos + needle.len()..];
    rest.find('"').map(|n| rest[..n].to_string()).unwrap_or_default()
}

fn text_of(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn dot(raw: &str) -> String {
    text_of(raw).replace(',', ".")
}

fn date(raw: &str) -> String {
    let t = text_of(raw);
    t.chars().take(10).collect()
}

fn empty_party() -> Party {
    Party {
        name: String::new(),
        eik: String::new(),
        vat: String::new(),
        country: String::new(),
    }
}

fn party<'a>(draft: &'a mut Draft, stack: &[Open]) -> Option<&'a mut Party> {
    if stack.iter().any(|s| s.name == "AccountingSupplierParty") {
        Some(&mut draft.supplier)
    } else if stack.iter().any(|s| s.name == "AccountingCustomerParty") {
        Some(&mut draft.customer)
    } else {
        None
    }
}

fn in_line(stack: &[Open]) -> bool {
    stack.iter().any(|s| s.name == "InvoiceLine" || s.name == "CreditNoteLine")
}

fn is_root(name: &str) -> bool {
    matches!(name, "Invoice" | "CreditNote" | "DebitNote")
}

pub fn parse(xml: &str) -> Result<Draft, String> {
    if !xml.contains('<') {
        return Err("Файлът не е XML.".into());
    }
    let mut draft = Draft {
        root: String::new(),
        number: String::new(),
        issue_date: String::new(),
        due_date: String::new(),
        tax_point: String::new(),
        type_code: String::new(),
        currency: String::new(),
        currency_rate: String::new(),
        pay_code: String::new(),
        note: String::new(),
        net: String::new(),
        vat: String::new(),
        total: String::new(),
        tax_cat: String::new(),
        exemption: String::new(),
        original: String::new(),
        supplier: empty_party(),
        customer: empty_party(),
        lines: Vec::new(),
    };
    let mut line = Line {
        qty: String::new(),
        unit: "C62".into(),
        net: String::new(),
        name: String::new(),
        seller: String::new(),
        price: String::new(),
        pct: String::new(),
    };
    let mut stack: Vec<Open> = Vec::new();
    let bytes = xml.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'<' {
            let start = i;
            while i < bytes.len() && bytes[i] != b'<' {
                i += 1;
            }
            let raw = &xml[start..i];
            let value = text_of(raw);
            if value.is_empty() {
                continue;
            }
            apply_text(&mut draft, &mut line, &stack, &value);
            continue;
        }
        if xml[i..].starts_with("</") {
            let Some(end) = xml[i..].find('>') else { break };
            let name = local(&xml[i + 2..i + end]);
            if name == "InvoiceLine" || name == "CreditNoteLine" {
                if !line.name.is_empty() || !line.net.is_empty() {
                    draft.lines.push(Line {
                        qty: if line.qty.is_empty() { "1".into() } else { line.qty.clone() },
                        unit: line.unit.clone(),
                        net: line.net.clone(),
                        name: line.name.clone(),
                        seller: line.seller.clone(),
                        price: line.price.clone(),
                        pct: line.pct.clone(),
                    });
                }
                line = Line {
                    qty: String::new(),
                    unit: "C62".into(),
                    net: String::new(),
                    name: String::new(),
                    seller: String::new(),
                    price: String::new(),
                    pct: String::new(),
                };
            }
            if stack.last().is_some_and(|s| s.name == name) {
                stack.pop();
            }
            i += end + 1;
            continue;
        }
        if xml[i..].starts_with("<?") || xml[i..].starts_with("<!") {
            let Some(end) = xml[i..].find('>') else { break };
            i += end + 1;
            continue;
        }
        let Some(end) = xml[i..].find('>') else { break };
        let body = &xml[i + 1..i + end];
        let closed = body.ends_with('/');
        let tag = body.trim_end_matches('/').trim();
        let name = local(tag);
        if draft.root.is_empty() && is_root(&name) {
            draft.root = name.clone();
        }
        if !closed {
            let unit = if name == "InvoicedQuantity" || name == "CreditedQuantity" {
                attr(tag, "unitCode")
            } else {
                String::new()
            };
            stack.push(Open { name, unit });
        }
        i += end + 1;
    }
    if draft.root.is_empty() {
        return Err("Файлът не е UBL фактура.".into());
    }
    Ok(draft)
}

fn apply_text(draft: &mut Draft, line: &mut Line, stack: &[Open], value: &str) {
    let Some(cur) = stack.last() else { return };
    let parent = stack.get(stack.len().wrapping_sub(2)).map(|s| s.name.as_str()).unwrap_or("");
    let tag = cur.name.as_str();
    if in_line(stack) {
        if tag == "InvoicedQuantity" || tag == "CreditedQuantity" {
            line.qty = dot(value);
            if !cur.unit.is_empty() {
                line.unit = cur.unit.clone();
            }
        }
        if tag == "LineExtensionAmount" {
            line.net = dot(value);
        }
        if tag == "Name" && parent == "Item" {
            line.name = value.to_string();
        }
        if tag == "Description" && parent == "Item" && line.name.is_empty() {
            line.name = value.to_string();
        }
        if tag == "ID" && parent == "SellersItemIdentification" {
            line.seller = value.to_string();
        }
        if tag == "PriceAmount" {
            line.price = dot(value);
        }
        if tag == "Percent" && parent == "ClassifiedTaxCategory" {
            line.pct = dot(value);
        }
        return;
    }
    if let Some(p) = party(draft, stack) {
        if tag == "Name" && parent == "PartyName" && p.name.is_empty() {
            p.name = value.to_string();
        }
        if tag == "RegistrationName" && p.name.is_empty() {
            p.name = value.to_string();
        }
        if tag == "CompanyID" && parent == "PartyTaxScheme" {
            p.vat = value.to_string();
        }
        if tag == "EndpointID" && p.vat.is_empty() {
            p.vat = value.to_string();
        }
        if tag == "ID" && parent == "PartyIdentification" {
            p.eik = value.to_string();
        }
        if tag == "CompanyID" && parent == "PartyLegalEntity" && p.eik.is_empty() {
            p.eik = value.to_string();
        }
        if tag == "IdentificationCode" && parent == "Country" {
            p.country = value.to_string();
        }
        return;
    }
    let root_parent = is_root(parent);
    if tag == "ID" && root_parent {
        draft.number = value.to_string();
    }
    if tag == "IssueDate" && root_parent {
        draft.issue_date = date(value);
    }
    if tag == "DueDate" && root_parent {
        draft.due_date = date(value);
    }
    if tag == "TaxPointDate" && root_parent {
        draft.tax_point = date(value);
    }
    if tag == "InvoiceTypeCode" || tag == "CreditNoteTypeCode" {
        draft.type_code = value.to_string();
    }
    if tag == "DocumentCurrencyCode" {
        draft.currency = value.to_string();
    }
    if tag == "PaymentMeansCode" {
        draft.pay_code = value.to_string();
    }
    if tag == "Note" && parent == "PaymentTerms" {
        draft.note = value.to_string();
    }
    if tag == "ID" && parent == "InvoiceDocumentReference" {
        draft.original = value.to_string();
    }
    if tag == "TaxAmount" && parent == "TaxTotal" && draft.vat.is_empty() {
        draft.vat = dot(value);
    }
    if tag == "LineExtensionAmount" && parent == "LegalMonetaryTotal" {
        draft.net = dot(value);
    }
    if tag == "TaxExclusiveAmount" && draft.net.is_empty() {
        draft.net = dot(value);
    }
    if tag == "PayableAmount" {
        draft.total = dot(value);
    }
    if tag == "TaxInclusiveAmount" && draft.total.is_empty() {
        draft.total = dot(value);
    }
    if tag == "CalculationRate" && draft.currency_rate.is_empty() {
        draft.currency_rate = dot(value);
    }
    if tag == "ID" && parent == "TaxCategory" && (draft.tax_cat.is_empty() || value != "S") {
        draft.tax_cat = value.to_string();
    }
    if tag == "TaxExemptionReasonCode" {
        draft.exemption = value.to_string();
    }
}

pub fn document_type(root: &str, code: &str) -> &'static str {
    let code = code.trim();
    if root == "CreditNote" || code == "381" || code == "396" {
        return "03";
    }
    if root == "DebitNote" || code == "383" {
        return "02";
    }
    if code == "325" {
        return "proforma";
    }
    "01"
}

pub fn pay_method(code: &str) -> &'static str {
    match code.trim() {
        "10" | "20" => "в брой",
        "48" | "54" | "55" => "карта",
        "97" => "компенсация",
        _ => "банков превод",
    }
}

pub fn exemption(raw: &str, category: &str) -> String {
    let code = raw.trim().to_ascii_uppercase();
    if !code.is_empty() {
        return code;
    }
    match category.trim() {
        "AE" => "VATEX-EU-AE".into(),
        "G" => "VATEX-EU-G".into(),
        "K" => "VATEX-EU-IC".into(),
        "O" => "VATEX-EU-O".into(),
        _ => String::new(),
    }
}
