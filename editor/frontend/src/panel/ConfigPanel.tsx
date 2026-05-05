import React from "react";
import type { IcNodeData, ComponentConfig } from "../types";

interface Props {
  data: IcNodeData;
  onChange: (updated: ComponentConfig) => void;
}

export default function ConfigPanel({ data, onChange }: Props) {
  const { instance, manifest } = data;
  const config = instance.config ?? {};
  const initVals = instance.initial_values ?? {};
  const mqttSub = instance.mqtt?.subscribe ?? {};
  const mqttPub = instance.mqtt?.publish ?? {};

  function setConfig(key: string, value: unknown) {
    onChange({ ...instance, config: { ...config, [key]: value } });
  }

  function setInitVal(key: string, value: string) {
    onChange({ ...instance, initial_values: { ...initVals, [key]: parseFloat(value) || value } });
  }

  function setMqttSub(channel: string, topic: string) {
    onChange({
      ...instance,
      mqtt: { ...instance.mqtt, subscribe: { ...mqttSub, [channel]: topic } },
    });
  }

  function setMqttPub(channel: string, topic: string) {
    onChange({
      ...instance,
      mqtt: { ...instance.mqtt, publish: { ...mqttPub, [channel]: topic } },
    });
  }

  function setFirmwarePath(path: string) {
    onChange({ ...instance, firmware: { transport: "unix_socket", path } });
  }

  return (
    <div style={{ padding: 12, fontFamily: "system-ui, sans-serif", fontSize: 13 }}>
      <h4 style={{ margin: "0 0 4px", fontSize: 14 }}>{instance.id}</h4>
      <div style={{ fontSize: 11, color: "#6b7280", marginBottom: 12 }}>{manifest.id} v{manifest.version}</div>

      {/* Bus interface config (e.g. I2C address) */}
      {manifest.interfaces.filter((i) => i.config).map((iface) => (
        <section key={iface.name} style={{ marginBottom: 10 }}>
          <Label>{iface.name} ({iface.protocol.toUpperCase()})</Label>
          {Object.entries(iface.config!).map(([field, spec]) => {
            const key = `${iface.protocol}.${field}`;
            const val = config[key] ?? spec.default;
            if (spec.choices) {
              return (
                <Row key={field} label={field}>
                  <select
                    value={String(val ?? "")}
                    onChange={(e) => setConfig(key, parseHexOrNum(e.target.value))}
                    style={INPUT_STYLE}
                  >
                    {spec.choices.map((c) => (
                      <option key={String(c)} value={String(c)}>
                        {spec.type === "hex" ? `0x${Number(c).toString(16).toUpperCase()}` : String(c)}
                      </option>
                    ))}
                  </select>
                </Row>
              );
            }
            return (
              <Row key={field} label={field}>
                <input
                  style={INPUT_STYLE}
                  defaultValue={spec.type === "hex" && typeof val === "number"
                    ? `0x${val.toString(16).toUpperCase()}` : String(val ?? "")}
                  onBlur={(e) => setConfig(key, parseHexOrNum(e.target.value))}
                />
              </Row>
            );
          })}
        </section>
      ))}

      {/* Initial values (sensors) */}
      {manifest.mqtt?.subscribe && manifest.mqtt.subscribe.length > 0 && (
        <section style={{ marginBottom: 10 }}>
          <Label>Initial values</Label>
          {manifest.mqtt.subscribe.map((ch) => (
            <Row key={ch.name} label={`${ch.name}${ch.unit ? ` (${ch.unit})` : ""}`}>
              <input
                style={INPUT_STYLE}
                defaultValue={String(initVals[ch.name] ?? "")}
                onBlur={(e) => setInitVal(ch.name, e.target.value)}
              />
            </Row>
          ))}
        </section>
      )}

      {/* MQTT subscribe topics */}
      {manifest.mqtt?.subscribe && manifest.mqtt.subscribe.length > 0 && (
        <section style={{ marginBottom: 10 }}>
          <Label>MQTT subscribe topics</Label>
          {manifest.mqtt.subscribe.map((ch) => (
            <Row key={ch.name} label={ch.name}>
              <input
                style={INPUT_STYLE}
                defaultValue={mqttSub[ch.name] ?? ""}
                onBlur={(e) => setMqttSub(ch.name, e.target.value)}
                placeholder="sensors/room1/temperature"
              />
            </Row>
          ))}
        </section>
      )}

      {/* MQTT publish topics */}
      {manifest.mqtt?.publish && manifest.mqtt.publish.length > 0 && (
        <section style={{ marginBottom: 10 }}>
          <Label>MQTT publish topics</Label>
          {manifest.mqtt.publish.map((ch) => (
            <Row key={ch.name} label={ch.name}>
              <input
                style={INPUT_STYLE}
                defaultValue={mqttPub[ch.name] ?? ""}
                onBlur={(e) => setMqttPub(ch.name, e.target.value)}
                placeholder="actuators/led1/state"
              />
            </Row>
          ))}
        </section>
      )}

      {/* Firmware socket (firmware_host) */}
      {manifest.kind === "firmware_host" && (
        <section style={{ marginBottom: 10 }}>
          <Label>Firmware</Label>
          <Row label="unix socket path">
            <input
              style={INPUT_STYLE}
              defaultValue={instance.firmware?.path ?? ""}
              onBlur={(e) => setFirmwarePath(e.target.value)}
              placeholder="/tmp/board_sim/mcu1.sock"
            />
          </Row>
        </section>
      )}
    </div>
  );
}

function Label({ children }: { children: React.ReactNode }) {
  return (
    <div style={{ fontWeight: 600, fontSize: 11, textTransform: "uppercase", color: "#374151", marginBottom: 4 }}>
      {children}
    </div>
  );
}

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div style={{ display: "flex", alignItems: "center", gap: 6, marginBottom: 4 }}>
      <span style={{ width: 120, color: "#6b7280", flexShrink: 0 }}>{label}</span>
      {children}
    </div>
  );
}

const INPUT_STYLE: React.CSSProperties = {
  flex: 1,
  padding: "2px 4px",
  border: "1px solid #d1d5db",
  borderRadius: 3,
  fontSize: 12,
  fontFamily: "monospace",
  width: "100%",
};

function parseHexOrNum(s: string): number | string {
  if (s.startsWith("0x") || s.startsWith("0X")) return parseInt(s, 16);
  const n = Number(s);
  return isNaN(n) ? s : n;
}
