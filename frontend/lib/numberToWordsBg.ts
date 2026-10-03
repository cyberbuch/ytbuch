// Словом на български за суми по фактура (лева/евро).

const ONES = ["", "един", "два", "три", "четири", "пет", "шест", "седем", "осем", "девет"];
const ONES_F = ["", "една", "две", "три", "четири", "пет", "шест", "седем", "осем", "девет"];
const TEENS = [
  "десет",
  "единадесет",
  "дванадесет",
  "тринадесет",
  "четиринадесет",
  "петнадесет",
  "шестнадесет",
  "седемнадесет",
  "осемнадесет",
  "деветнадесет",
];
const TENS = [
  "",
  "",
  "двадесет",
  "тридесет",
  "четиридесет",
  "петдесет",
  "шестдесет",
  "седемдесет",
  "осемдесет",
  "деветдесет",
];
const HUNDREDS = [
  "",
  "сто",
  "двеста",
  "триста",
  "четиристотин",
  "петстотин",
  "шестстотин",
  "седемстотин",
  "осемстотин",
  "деветстотин",
];

function hundreds(n: number, female: boolean): string {
  const ones = female ? ONES_F : ONES;
  if (n === 0) return "";
  if (n < 10) return ones[n];
  if (n < 20) return TEENS[n - 10];
  if (n < 100) {
    const ten = Math.floor(n / 10);
    const one = n % 10;
    if (one === 0) return TENS[ten];
    return TENS[ten] + " и " + ones[one];
  }
  const h = Math.floor(n / 100);
  const rest = n % 100;
  if (rest === 0) return HUNDREDS[h];
  if (rest < 10) return HUNDREDS[h] + " и " + ones[rest];
  if (rest < 20) return HUNDREDS[h] + " и " + TEENS[rest - 10];
  const ten = Math.floor(rest / 10);
  const one = rest % 10;
  if (one === 0) return HUNDREDS[h] + " и " + TENS[ten];
  return HUNDREDS[h] + " " + TENS[ten] + " и " + ones[one];
}

function belowMillion(n: number, female: boolean): string {
  if (n === 0) return "";
  if (n < 1000) return hundreds(n, female);
  const th = Math.floor(n / 1000);
  const rest = n % 1000;
  let head = "";
  if (th === 1) head = "хиляда";
  else if (th === 2) head = "две хиляди";
  else head = hundreds(th, true) + " хиляди";
  if (rest === 0) return head;
  if (rest < 100) return head + " и " + hundreds(rest, female);
  return head + " " + hundreds(rest, female);
}

function integerWords(n: number, female: boolean): string {
  if (n === 0) return female ? "нула" : "нула";
  if (n < 1_000_000) return belowMillion(n, female);
  const mil = Math.floor(n / 1_000_000);
  const rest = n % 1_000_000;
  const head = mil === 1 ? "един милион" : hundreds(mil, false) + " милиона";
  if (rest === 0) return head;
  if (rest < 1000) return head + " и " + belowMillion(rest, female);
  return head + " " + belowMillion(rest, female);
}

export function numberToWordsBg(amount: string | number, currency = "BGN"): string {
  const n = typeof amount === "number" ? amount : parseFloat(String(amount).replace(",", ".")) || 0;
  const neg = n < 0;
  const abs = Math.abs(n);
  const parts = abs.toFixed(2).split(".");
  const whole = parseInt(parts[0], 10);
  const cents = parseInt(parts[1], 10);
  const female = currency === "BGN";
  let cur = currency;
  let centName = "стотинки";
  if (currency === "BGN") {
    cur = whole === 1 ? "лев" : "лева";
    centName = "стотинки";
  } else if (currency === "EUR") {
    cur = whole === 1 ? "евро" : "евро";
    centName = "евроцента";
  }
  let s = integerWords(whole, female) + " " + cur;
  if (cents > 0) {
    s += " и " + String(cents).padStart(2, "0") + " " + centName;
  }
  if (neg) s = "минус " + s;
  return s;
}
