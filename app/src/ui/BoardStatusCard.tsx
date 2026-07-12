// SPDX-License-Identifier: GPL-2.0-or-later
import { formatProtocolVersion } from '../kcp';
import { Field, Panel } from './Panel';
import type { DeviceSnapshot } from './useKcpDevice';

interface BoardStatusCardProps {
  snapshot: DeviceSnapshot;
}

/** Operator-facing board identity from the INFO snapshot: protocol, firmware, chip and config schema. */
export function BoardStatusCard({ snapshot }: BoardStatusCardProps) {
  const fields: { label: string; value: string }[] = [
    { label: 'Protocol', value: formatProtocolVersion(snapshot.protocolVersion) },
    { label: 'Firmware', value: snapshot.deviceInfo.firmwareVersionString },
    { label: 'Chip', value: snapshot.deviceInfo.chip },
    { label: 'Schema', value: String(snapshot.deviceInfo.schemaVersion) },
  ];

  return (
    <Panel title="Board">
      <dl className="kb-field-grid">
        {fields.map((field) => (
          <Field key={field.label} label={field.label} value={field.value} />
        ))}
      </dl>
    </Panel>
  );
}
