// Dados públicos de recebimento das contribuições voluntárias (PIX).
// Preencha `pixKey` e `city`; enquanto estiverem vazios a aba Contribuir
// mostra um aviso em vez de gerar um QR inválido.
export const CONTRIBUTION = {
  pixKey: "joaopanuccj@gmail.com",
  receiverName: "João Paulo Panucci Bernardes",
  city: "Campo Grande",
  repository: "https://github.com/joaopanucci/autodock",
  suggestedAmounts: [10, 25, 50],
} as const;
