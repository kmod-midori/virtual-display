#include "Driver.h"
#include "Driver.tmh"

using namespace Microsoft::IndirectDisp;
using namespace Microsoft::WRL;

IndirectDeviceContext::IndirectDeviceContext(_In_ WDFDEVICE WdfDevice) :
  m_WdfDevice(WdfDevice) {
  m_Adapter = {};
}

IndirectDeviceContext::~IndirectDeviceContext() {
}

void IndirectDeviceContext::InitAdapter() {
  // ==============================
  // TODO: Update the below diagnostic information in accordance with the target hardware. The strings and version
  // numbers are used for telemetry and may be displayed to the user in some situations.
  //
  // This is also where static per-adapter capabilities are determined.
  // ==============================

  IDDCX_ADAPTER_CAPS AdapterCaps = {};
  AdapterCaps.Size = sizeof(AdapterCaps);

  // Declare basic feature support for the adapter (required)
  AdapterCaps.MaxMonitorsSupported = IDD_SAMPLE_MONITOR_COUNT;
  AdapterCaps.EndPointDiagnostics.Size = sizeof(AdapterCaps.EndPointDiagnostics);
  AdapterCaps.EndPointDiagnostics.GammaSupport = IDDCX_FEATURE_IMPLEMENTATION_NONE;
  AdapterCaps.EndPointDiagnostics.TransmissionType = IDDCX_TRANSMISSION_TYPE_WIRELESS_WIFI_OTHER;

  // Declare your device strings for telemetry (required)
  AdapterCaps.EndPointDiagnostics.pEndPointFriendlyName = L"VirtualDisplay Device";
  AdapterCaps.EndPointDiagnostics.pEndPointManufacturerName = L"MyCorp, Inc";
  AdapterCaps.EndPointDiagnostics.pEndPointModelName = L"VirtualDisplay Model";

  // Declare your hardware and firmware versions (required)
  IDDCX_ENDPOINT_VERSION Version = {};
  Version.Size = sizeof(Version);
  Version.MajorVer = 1;
  AdapterCaps.EndPointDiagnostics.pFirmwareVersion = &Version;
  AdapterCaps.EndPointDiagnostics.pHardwareVersion = &Version;

  // Initialize a WDF context that can store a pointer to the device context object
  WDF_OBJECT_ATTRIBUTES Attr;
  WDF_OBJECT_ATTRIBUTES_INIT_CONTEXT_TYPE(&Attr, IndirectDeviceContextWrapper);

  IDARG_IN_ADAPTER_INIT AdapterInit = {};
  AdapterInit.WdfDevice = m_WdfDevice;
  AdapterInit.pCaps = &AdapterCaps;
  AdapterInit.ObjectAttributes = &Attr;

  // Start the initialization of the adapter, which will trigger the AdapterFinishInit callback later
  IDARG_OUT_ADAPTER_INIT AdapterInitOut;
  NTSTATUS Status = IddCxAdapterInitAsync(&AdapterInit, &AdapterInitOut);

  if (NT_SUCCESS(Status)) {
    // Store a reference to the WDF adapter handle
    m_Adapter = AdapterInitOut.AdapterObject;

    // Store the device context object into the WDF object context
    auto* pContext = WdfObjectGet_IndirectDeviceContextWrapper(AdapterInitOut.AdapterObject);
    pContext->pContext = this;
  }
}

void IndirectDeviceContext::CreateMonitor(UINT ConnectorIndex) {
  // ==============================
  // TODO: In a real driver, the EDID should be retrieved dynamically from a connected physical monitor. The EDIDs
  // provided here are purely for demonstration.
  // Monitor manufacturers are required to correctly fill in physical monitor attributes in order to allow the OS
  // to optimize settings like viewing distance and scale factor. Manufacturers should also use a unique serial
  // number every single device to ensure the OS can tell the monitors apart.
  // ==============================

  WDF_OBJECT_ATTRIBUTES Attr;
  WDF_OBJECT_ATTRIBUTES_INIT_CONTEXT_TYPE(&Attr, IndirectMonitorContextWrapper);
  Attr.EvtCleanupCallback = [](WDFOBJECT Object) {
    auto* pContext = WdfObjectGet_IndirectMonitorContextWrapper(Object);
    if (pContext) {
      pContext->Cleanup();
    }
  };


  // In the sample driver, we report a monitor right away but a real driver would do this when a monitor connection event occurs
  IDDCX_MONITOR_INFO MonitorInfo = {};
  MonitorInfo.Size = sizeof(MonitorInfo);
  MonitorInfo.MonitorType = DISPLAYCONFIG_OUTPUT_TECHNOLOGY_HDMI;
  MonitorInfo.ConnectorIndex = ConnectorIndex;

  MonitorInfo.MonitorDescription.Size = sizeof(MonitorInfo.MonitorDescription);
  MonitorInfo.MonitorDescription.Type = IDDCX_MONITOR_DESCRIPTION_TYPE_EDID;
  MonitorInfo.MonitorDescription.DataSize = 0;
  MonitorInfo.MonitorDescription.pData = nullptr;

  // ==============================
  // TODO: The monitor's container ID should be distinct from "this" device's container ID if the monitor is not
  // permanently attached to the display adapter device object. The container ID is typically made unique for each
  // monitor and can be used to associate the monitor with other devices, like audio or input devices. In this
  // sample we generate a random container ID GUID, but it's best practice to choose a stable container ID for a
  // unique monitor or to use "this" device's container ID for a permanent/integrated monitor.
  // ==============================

  // Create a container ID
  CoCreateGuid(&MonitorInfo.MonitorContainerId);

  IDARG_IN_MONITORCREATE MonitorCreate = {};
  MonitorCreate.ObjectAttributes = &Attr;
  MonitorCreate.pMonitorInfo = &MonitorInfo;

  // Create a monitor object with the specified monitor descriptor
  IDARG_OUT_MONITORCREATE MonitorCreateOut;
  NTSTATUS Status = IddCxMonitorCreate(m_Adapter, &MonitorCreate, &MonitorCreateOut);
  if (NT_SUCCESS(Status)) {
    // Create a new monitor context object and attach it to the Idd monitor object
    auto* pMonitorContextWrapper = WdfObjectGet_IndirectMonitorContextWrapper(MonitorCreateOut.MonitorObject);
    pMonitorContextWrapper->pContext = new IndirectMonitorContext(MonitorCreateOut.MonitorObject, ConnectorIndex);

    // Tell the OS that the monitor has been plugged in
    IDARG_OUT_MONITORARRIVAL ArrivalOut;
    Status = IddCxMonitorArrival(MonitorCreateOut.MonitorObject, &ArrivalOut);
  }
}
