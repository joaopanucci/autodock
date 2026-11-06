// PIX "copia e cola" (BR Code estático, EMV QRCPS-MPM). Sem rede e sem dependências.
export type PixInput = {
  key: string;
  name: string;
  city: string;
  amount?: number;
  txid?: string;
};

// Normaliza para ASCII maiúsculo, como recomendado pelo Manual do BR Code.
function clean(value: string, max: number) {
  return value
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "")
    .replace(/[^A-Za-z0-9 .,\-]/g, "")
    .trim()
    .toUpperCase()
    .slice(0, max);
}

function field(id: string, value: string) {
  if (value.length > 99) throw new Error("Campo PIX excede o tamanho permitido.");
  return `${id}${String(value.length).padStart(2, "0")}${value}`;
}

export function crc16(text: string) {
  let crc = 0xffff;
  for (let i = 0; i < text.length; i++) {
    crc ^= text.charCodeAt(i) << 8;
    for (let bit = 0; bit < 8; bit++) {
      crc = crc & 0x8000 ? ((crc << 1) ^ 0x1021) & 0xffff : (crc << 1) & 0xffff;
    }
  }
  return crc.toString(16).toUpperCase().padStart(4, "0");
}

export function pixPayload({ key, name, city, amount, txid }: PixInput) {
  const pixKey = key.trim();
  if (!pixKey || /[^\x21-\x7e]/.test(pixKey)) throw new Error("Chave PIX inválida.");
  const receiver = clean(name, 25);
  const town = clean(city, 15);
  if (!receiver || !town) throw new Error("Nome e cidade do recebedor são obrigatórios.");
  if (amount !== undefined && !(amount > 0 && amount < 1_000_000)) {
    throw new Error("Valor inválido.");
  }
  const reference = clean(txid ?? "", 25).replace(/[^A-Z0-9]/g, "") || "***";
  const body =
    field("00", "01") +
    field("01", "11") +
    field("26", field("00", "br.gov.bcb.pix") + field("01", pixKey)) +
    field("52", "0000") +
    field("53", "986") +
    (amount !== undefined ? field("54", amount.toFixed(2)) : "") +
    field("58", "BR") +
    field("59", receiver) +
    field("60", town) +
    field("62", field("05", reference)) +
    "6304";
  return body + crc16(body);
}
