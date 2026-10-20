package dev.teamy.broker.dummy;
/** Bounded whole operations, including server identity checks; no idle/traffic lease renewal. */
final class PhoneTiming {
 static final int POLL_MS=2000,TCP_MS=3000,CONNECT_MS=25000,EXCHANGE_MS=30000,OUTBOUND_MS=5000;
 private PhoneTiming(){}
}
