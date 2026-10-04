//! Каталог на бланките по СС 1 / СС 7 и стандартните формули.
//! Ред на бланка: code|parent|kind|side|name. Формула: line_code|formula.

const BALANCE: &[&str] = &[
    "bal.a||line|asset|А. Записан, но невнесен капитал",
    "bal.b||header|asset|Б. Нетекущи (дълготрайни) активи",
    "bal.b.i|bal.b|header|asset|I. Нематериални активи",
    "bal.b.i.1|bal.b.i|line|asset|1. Продукти от развойна дейност",
    "bal.b.i.2|bal.b.i|line|asset|2. Концесии, патенти, лицензии, търговски марки, програмни продукти",
    "bal.b.i.3|bal.b.i|line|asset|3. Търговска репутация",
    "bal.b.i.4|bal.b.i|line|asset|4. Предоставени аванси и нематериални активи в процес на изграждане",
    "bal.b.i.t|bal.b.i|total|asset|Общо за група I",
    "bal.b.ii|bal.b|header|asset|II. Дълготрайни материални активи",
    "bal.b.ii.1|bal.b.ii|line|asset|1. Земи и сгради",
    "bal.b.ii.1a|bal.b.ii.1|line|asset|— земи",
    "bal.b.ii.1b|bal.b.ii.1|line|asset|— сгради",
    "bal.b.ii.2|bal.b.ii|line|asset|2. Машини, производствено оборудване и апаратура",
    "bal.b.ii.3|bal.b.ii|line|asset|3. Съоръжения и други",
    "bal.b.ii.4|bal.b.ii|line|asset|4. Предоставени аванси и ДМА в процес на изграждане",
    "bal.b.ii.t|bal.b.ii|total|asset|Общо за група II",
    "bal.b.iii|bal.b|header|asset|III. Дългосрочни финансови активи",
    "bal.b.iii.1|bal.b.iii|line|asset|1. Акции и дялове в предприятия от група",
    "bal.b.iii.5|bal.b.iii|line|asset|5. Дългосрочни инвестиции",
    "bal.b.iii.6|bal.b.iii|line|asset|6. Други заеми",
    "bal.b.iii.t|bal.b.iii|total|asset|Общо за група III",
    "bal.b.iv|bal.b|line|asset|IV. Отсрочени данъци",
    "bal.b.t|bal.b|total|asset|Общо за раздел Б",
    "bal.c||header|asset|В. Текущи (краткотрайни) активи",
    "bal.c.i|bal.c|header|asset|I. Материални запаси",
    "bal.c.i.1|bal.c.i|line|asset|1. Суровини и материали",
    "bal.c.i.2|bal.c.i|line|asset|2. Незавършено производство",
    "bal.c.i.3|bal.c.i|line|asset|3. Продукция и стоки",
    "bal.c.i.3a|bal.c.i.3|line|asset|— продукция",
    "bal.c.i.3b|bal.c.i.3|line|asset|— стоки",
    "bal.c.i.4|bal.c.i|line|asset|4. Предоставени аванси",
    "bal.c.i.t|bal.c.i|total|asset|Общо за група I",
    "bal.c.ii|bal.c|header|asset|II. Вземания",
    "bal.c.ii.1|bal.c.ii|line|asset|1. Вземания от клиенти и доставчици",
    "bal.c.ii.4|bal.c.ii|line|asset|4. Други вземания",
    "bal.c.ii.t|bal.c.ii|total|asset|Общо за група II",
    "bal.c.iii|bal.c|header|asset|III. Инвестиции",
    "bal.c.iii.3|bal.c.iii|line|asset|3. Други инвестиции",
    "bal.c.iii.t|bal.c.iii|total|asset|Общо за група III",
    "bal.c.iv|bal.c|header|asset|IV. Парични средства",
    "bal.c.iv.a|bal.c.iv|line|asset|— в брой",
    "bal.c.iv.b|bal.c.iv|line|asset|— в безсрочни сметки (депозити)",
    "bal.c.iv.t|bal.c.iv|total|asset|Общо за група IV",
    "bal.c.t|bal.c|total|asset|Общо за раздел В",
    "bal.d||line|asset|Г. Разходи за бъдещи периоди",
    "bal.t||total|asset|Сума на актива (А+Б+В+Г)",
    "bal.p.a||header|liability|А. Собствен капитал",
    "bal.p.a.i|bal.p.a|line|liability|I. Записан капитал",
    "bal.p.a.ii|bal.p.a|line|liability|II. Премии от емисии",
    "bal.p.a.iii|bal.p.a|line|liability|III. Резерв от последващи оценки",
    "bal.p.a.iv|bal.p.a|header|liability|IV. Резерви",
    "bal.p.a.iv.1|bal.p.a.iv|line|liability|1. Законови резерви",
    "bal.p.a.iv.4|bal.p.a.iv|line|liability|4. Други резерви",
    "bal.p.a.iv.t|bal.p.a.iv|total|liability|Общо за група IV",
    "bal.p.a.v|bal.p.a|header|liability|V. Натрупана печалба (загуба) от минали години",
    "bal.p.a.v.1|bal.p.a.v|line|liability|— неразпределена печалба",
    "bal.p.a.v.2|bal.p.a.v|line|liability|— непокрита загуба",
    "bal.p.a.v.t|bal.p.a.v|total|liability|Общо за група V",
    "bal.p.a.vi|bal.p.a|line|liability|VI. Текуща печалба (загуба)",
    "bal.p.a.t|bal.p.a|total|liability|Общо за раздел А",
    "bal.p.b||header|liability|Б. Провизии и сходни задължения",
    "bal.p.b.1|bal.p.b|line|liability|1. Провизии за пенсии и други подобни задължения",
    "bal.p.b.2|bal.p.b|line|liability|2. Провизии за данъци",
    "bal.p.b.3|bal.p.b|line|liability|3. Други провизии и сходни задължения",
    "bal.p.b.t|bal.p.b|total|liability|Общо за раздел Б",
    "bal.p.c||header|liability|В. Задължения",
    "bal.p.c.2|bal.p.c|line|liability|2. Задължения към финансови предприятия",
    "bal.p.c.3|bal.p.c|line|liability|3. Получени аванси",
    "bal.p.c.4|bal.p.c|line|liability|4. Задължения към доставчици",
    "bal.p.c.8|bal.p.c|line|liability|8. Други задължения",
    "bal.p.c.t|bal.p.c|total|liability|Общо за раздел В",
    "bal.p.d||line|liability|Г. Финансирания и приходи за бъдещи периоди",
    "bal.p.t||total|liability|Сума на пасива (А+Б+В+Г)",
];

const BALANCE_DEF: &[&str] = &[
    "bal.b.ii.1a|+201Д",
    "bal.b.ii.1b|+202Д-242К",
    "bal.b.ii.1|+201Д+202Д-242К",
    "bal.b.ii.2|+203Д+204Д+205Д-241К",
    "bal.b.ii.3|+206Д+207Д+209Д",
    "bal.b.ii.4|+207Д",
    "bal.b.iii.5|+221Д+222Д",
    "bal.b.iii.6|+226Д",
    "bal.b.iv|+242Д",
    "bal.b.t|+@bal.b.i.t+@bal.b.ii.t+@bal.b.iii.t+@bal.b.iv",
    "bal.c.i.1|+302Д+301Д",
    "bal.c.i.2|+611Д",
    "bal.c.i.3a|+303Д",
    "bal.c.i.3b|+304Д",
    "bal.c.i.3|+303Д+304Д",
    "bal.c.i.4|+302Д",
    "bal.c.ii.1|+411Д+413Д",
    "bal.c.ii.4|+412Д+421Д+422Д+4531Д",
    "bal.c.iii.3|+231Д+232Д",
    "bal.c.iv.a|+501Д+502Д",
    "bal.c.iv.b|+503Д+504Д+505Д+506Д",
    "bal.c.t|+@bal.c.i.t+@bal.c.ii.t+@bal.c.iii.t+@bal.c.iv.t",
    "bal.d|+651Д",
    "bal.t|+@bal.a+@bal.b.t+@bal.c.t+@bal.d",
    "bal.p.a.i|+101К",
    "bal.p.a.ii|+102К",
    "bal.p.a.iii|+112К",
    "bal.p.a.iv.1|+111К",
    "bal.p.a.iv.4|+119К",
    "bal.p.a.v.1|+122К",
    "bal.p.a.v.2|+121Д",
    "bal.p.a.vi|+123К-123Д",
    "bal.p.a.t|+@bal.p.a.i+@bal.p.a.ii+@bal.p.a.iii+@bal.p.a.iv.t+@bal.p.a.v.t+@bal.p.a.vi",
    "bal.p.b.2|+496К",
    "bal.p.b.3|+498К",
    "bal.p.c.2|+151К+152К",
    "bal.p.c.3|+412К",
    "bal.p.c.4|+401К+402К",
    "bal.p.c.8|+421К+422К+424К+425К+4532К+455К+459К",
    "bal.p.d|+151К+496К",
    "bal.p.t|+@bal.p.a.t+@bal.p.b.t+@bal.p.c.t+@bal.p.d",
];

const PL: &[&str] = &[
    "pl.e||header|expense|А. Разходи",
    "pl.e.1|pl.e|line|expense|1. Намаление на запасите от продукция и незавършено производство",
    "pl.e.2|pl.e|line|expense|2. Разходи за суровини, материали и външни услуги",
    "pl.e.2a|pl.e.2|line|expense|а) суровини и материали",
    "pl.e.2b|pl.e.2|line|expense|б) външни услуги",
    "pl.e.3|pl.e|line|expense|3. Разходи за персонала",
    "pl.e.3a|pl.e.3|line|expense|а) разходи за възнаграждения",
    "pl.e.3b|pl.e.3|line|expense|б) разходи за осигуровки",
    "pl.e.4|pl.e|line|expense|4. Разходи за амортизация и обезценка",
    "pl.e.5|pl.e|line|expense|5. Други разходи",
    "pl.e.5a|pl.e.5|line|expense|а) балансова стойност на продадените активи",
    "pl.e.op|pl.e|total|expense|Общо разходи за оперативна дейност",
    "pl.e.6|pl.e|line|expense|6. Разходи от обезценка на финансови активи",
    "pl.e.7|pl.e|line|expense|7. Разходи за лихви и други финансови разходи",
    "pl.e.fin|pl.e|total|expense|Общо финансови разходи",
    "pl.e.t|pl.e|total|expense|Общо разходи",
    "pl.e.tax|pl.e|line|expense|10. Разходи за данъци от печалбата",
    "pl.e.profit|pl.e|total|expense|12. Печалба",
    "pl.i||header|income|Б. Приходи",
    "pl.i.1|pl.i|line|income|1. Нетни приходи от продажби",
    "pl.i.1a|pl.i.1|line|income|а) продукция",
    "pl.i.1b|pl.i.1|line|income|б) стоки",
    "pl.i.1c|pl.i.1|line|income|в) услуги",
    "pl.i.2|pl.i|line|income|2. Увеличение на запасите от продукция и незавършено производство",
    "pl.i.3|pl.i|line|income|3. Разходи за придобиване на активи по стопански начин",
    "pl.i.4|pl.i|line|income|4. Други приходи",
    "pl.i.op|pl.i|total|income|Общо приходи от оперативна дейност",
    "pl.i.5|pl.i|line|income|5. Приходи от участия",
    "pl.i.6|pl.i|line|income|6. Приходи от други инвестиции и заеми",
    "pl.i.7|pl.i|line|income|7. Други лихви и финансови приходи",
    "pl.i.fin|pl.i|total|income|Общо финансови приходи",
    "pl.i.t|pl.i|total|income|Общо приходи",
    "pl.i.loss|pl.i|total|income|10. Загуба",
];

const PL_DEF: &[&str] = &[
    "pl.e.2a|+601ОД",
    "pl.e.2b|+602ОД",
    "pl.e.2|+601ОД+602ОД",
    "pl.e.3a|+604ОД",
    "pl.e.3b|+605ОД",
    "pl.e.3|+604ОД+605ОД",
    "pl.e.4|+603ОД",
    "pl.e.5a|+702Д/304К+701Д/303К",
    "pl.e.5|+609ОД+@pl.e.5a",
    "pl.e.op|+@pl.e.1+@pl.e.2+@pl.e.3+@pl.e.4+@pl.e.5",
    "pl.e.6|+624ОД",
    "pl.e.7|+621ОД+629ОД",
    "pl.e.fin|+@pl.e.6+@pl.e.7",
    "pl.e.t|+@pl.e.op+@pl.e.fin",
    "pl.e.tax|+608ОД",
    "pl.i.1a|+701ОК",
    "pl.i.1b|+702ОК",
    "pl.i.1c|+703ОК",
    "pl.i.1|+701ОК+702ОК+703ОК",
    "pl.i.4|+709ОК",
    "pl.i.op|+@pl.i.1+@pl.i.2+@pl.i.3+@pl.i.4",
    "pl.i.7|+721ОК+724ОК",
    "pl.i.fin|+@pl.i.5+@pl.i.6+@pl.i.7",
    "pl.i.t|+@pl.i.op+@pl.i.fin",
    "pl.e.profit|+@pl.i.t-@pl.e.t-@pl.e.tax",
    "pl.i.loss|+@pl.e.t-@pl.i.t",
];

const CASH: &[&str] = &[
    "cf.op||header|cash|А. Оперативна дейност",
    "cf.op.in.cust|cf.op|line|cash|Постъпления от клиенти",
    "cf.op.out.sup|cf.op|line|cash|Плащания към доставчици",
    "cf.op.out.staff|cf.op|line|cash|Плащания за персонал",
    "cf.op.out.tax|cf.op|line|cash|Платени данъци",
    "cf.op.out.other|cf.op|line|cash|Други плащания по оперативна дейност",
    "cf.op.net|cf.op|total|cash|Нетен поток от оперативна дейност",
    "cf.inv||header|cash|Б. Инвестиционна дейност",
    "cf.inv.out.ppe|cf.inv|line|cash|Придобиване на дълготрайни активи",
    "cf.inv.in.ppe|cf.inv|line|cash|Постъпления от продажба на дълготрайни активи",
    "cf.inv.net|cf.inv|total|cash|Нетен поток от инвестиционна дейност",
    "cf.fin||header|cash|В. Финансова дейност",
    "cf.fin.in.cap|cf.fin|line|cash|Постъпления от собственици",
    "cf.fin.in.loan|cf.fin|line|cash|Получени заеми",
    "cf.fin.out.loan|cf.fin|line|cash|Погасени заеми",
    "cf.fin.out.div|cf.fin|line|cash|Платени дивиденти",
    "cf.fin.net|cf.fin|total|cash|Нетен поток от финансова дейност",
    "cf.change||total|cash|Изменение на паричните средства",
    "cf.open||line|cash|Парични средства в началото на периода",
    "cf.close||total|cash|Парични средства в края на периода",
];

const CASH_DEF: &[&str] = &[
    "cf.op.in.cust|+50*Д/411К",
    "cf.op.out.sup|+401Д/50*К",
    "cf.op.out.staff|+421Д/50*К+604Д/50*К",
    "cf.op.out.tax|+4532Д/50*К+459Д/50*К",
    "cf.op.net|+@cf.op.in.cust-@cf.op.out.sup-@cf.op.out.staff-@cf.op.out.tax-@cf.op.out.other",
    "cf.inv.out.ppe|+20*Д/50*К",
    "cf.inv.in.ppe|+50*Д/20*К",
    "cf.inv.net|+@cf.inv.in.ppe-@cf.inv.out.ppe",
    "cf.fin.in.cap|+50*Д/101К",
    "cf.fin.in.loan|+50*Д/151К",
    "cf.fin.out.loan|+151Д/50*К",
    "cf.fin.out.div|+455Д/50*К",
    "cf.fin.net|+@cf.fin.in.cap+@cf.fin.in.loan-@cf.fin.out.loan-@cf.fin.out.div",
    "cf.change|+@cf.op.net+@cf.inv.net+@cf.fin.net",
    "cf.open|+501НД+502НД+503НД",
    "cf.close|+@cf.open+@cf.change",
];

const EQUITY: &[&str] = &[
    "eq.open||line|equity|1. Салдо в началото на отчетния период",
    "eq.policy||line|equity|2. Промени в счетоводната политика",
    "eq.errors||line|equity|3. Грешки",
    "eq.after||total|equity|4. Салдо след промени в политиката и грешки",
    "eq.owners||line|equity|5. Изменения за сметка на собствениците",
    "eq.result||line|equity|6. Финансов резултат за текущия период",
    "eq.div||line|equity|7. Разпределения на печалба (дивиденти)",
    "eq.reval||line|equity|9. Последващи оценки на активи и пасиви",
    "eq.other||line|equity|10. Други изменения в собствения капитал",
    "eq.close||total|equity|11. Салдо към края на отчетния период",
];

const EQUITY_DEF: &[&str] = &[
    "eq.open|+101НК+111НК+112НК+119НК+122НК-121НД",
    "eq.after|+@eq.open+@eq.policy+@eq.errors",
    "eq.owners|+101К-101НК",
    "eq.result|+123К-123Д",
    "eq.div|+123Д/455К",
    "eq.close|+@eq.after+@eq.owners+@eq.result-@eq.div+@eq.reval+@eq.other",
];


pub struct Line {
    pub code: String,
    pub parent: String,
    pub kind: String,
    pub side: String,
    pub name: String,
}

pub fn known(statement: &str) -> bool {
    matches!(statement, "balance" | "pl" | "cashflow" | "equity")
}

fn table(statement: &str) -> &'static [&'static str] {
    match statement {
        "balance" => BALANCE,
        "pl" => PL,
        "cashflow" => CASH,
        "equity" => EQUITY,
        _ => &[],
    }
}

fn defaults_table(statement: &str) -> &'static [&'static str] {
    match statement {
        "balance" => BALANCE_DEF,
        "pl" => PL_DEF,
        "cashflow" => CASH_DEF,
        "equity" => EQUITY_DEF,
        _ => &[],
    }
}

pub fn lines(statement: &str) -> Vec<Line> {
    table(statement).iter().map(|row| parse_line(row)).collect()
}

pub fn parse_line(row: &str) -> Line {
    let p: Vec<&str> = row.split('|').collect();
    let at = |i: usize| p.get(i).copied().unwrap_or("").to_string();
    Line { code: at(0), parent: at(1), kind: at(2), side: at(3), name: at(4) }
}

pub fn has_code(statement: &str, code: &str) -> bool {
    lines(statement).iter().any(|l| l.code == code)
}

pub fn defaults(statement: &str) -> Vec<(String, String)> {
    defaults_table(statement)
        .iter()
        .filter_map(|row| {
            let (code, formula) = row.split_once('|')?;
            Some((code.to_string(), formula.to_string()))
        })
        .collect()
}

pub fn statements() -> Vec<(&'static str, &'static str)> {
    vec![
        ("balance", "Счетоводен баланс"),
        ("pl", "Отчет за приходите и разходите"),
        ("cashflow", "Отчет за паричните потоци"),
        ("equity", "Отчет за собствения капитал"),
    ]
}
