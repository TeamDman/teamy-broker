package dev.teamy.broker.dummy;
interface SupplyRequest {String id();boolean live();boolean supplyAllowed();boolean fixture();boolean submitted();void markSubmitted();}
