export interface Qso {
  id: string;
  ts: number; // unix ms
  call: string;
  freqHz: number;
  band: string;
  mode: string;
  rstSent: string;
  rstRcvd: string;
  exchSent: string;
  exchRcvd: string;
  serialSent: number;
}
