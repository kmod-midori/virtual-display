/*++

Copyright (c) Microsoft Corporation

Abstract:

    This module contains a sample implementation of an indirect display driver. See the included README.md file and the
    various TODO blocks throughout this file and all accompanying files for information on building a production driver.

    MSDN documentation on indirect displays can be found at https://msdn.microsoft.com/en-us/library/windows/hardware/mt761968(v=vs.85).aspx.

Environment:

    User Mode, UMDF

--*/

#include "Driver.h"
#include "Driver.tmh"


using namespace std;
using namespace Microsoft::IndirectDisp;
using namespace Microsoft::WRL;

#pragma region SampleMonitors

// Default modes reported for edid-less monitors. The first mode is set as preferred
static const struct IndirectSampleMonitor::SampleMonitorMode s_DefaultModes[] =
{
    { 2560, 1440, 60 },
    { 1920, 1200, 60 },
    { 1920, 1080, 60 },
    { 1600,  900, 60 },
    { 1024,  768, 60 },
};

#pragma endregion

#pragma region helpers

static inline void FillSignalInfo(DISPLAYCONFIG_VIDEO_SIGNAL_INFO& Mode, DWORD Width, DWORD Height, DWORD VSync, bool bMonitorMode) {
  Mode.totalSize.cx = Mode.activeSize.cx = Width;
  Mode.totalSize.cy = Mode.activeSize.cy = Height;

  // See https://docs.microsoft.com/en-us/windows/win32/api/wingdi/ns-wingdi-displayconfig_video_signal_info
  Mode.AdditionalSignalInfo.vSyncFreqDivider = bMonitorMode ? 0 : 1;
  Mode.AdditionalSignalInfo.videoStandard = 255;

  Mode.vSyncFreq.Numerator = VSync;
  Mode.vSyncFreq.Denominator = 1;
  Mode.hSyncFreq.Numerator = VSync * Height;
  Mode.hSyncFreq.Denominator = 1;

  Mode.scanLineOrdering = DISPLAYCONFIG_SCANLINE_ORDERING_PROGRESSIVE;

  Mode.pixelRate = ((UINT64)VSync) * ((UINT64)Width) * ((UINT64)Height);
}

static IDDCX_MONITOR_MODE CreateIddCxMonitorMode(DWORD Width, DWORD Height, DWORD VSync, IDDCX_MONITOR_MODE_ORIGIN Origin = IDDCX_MONITOR_MODE_ORIGIN_DRIVER) {
  IDDCX_MONITOR_MODE Mode = {};

  Mode.Size = sizeof(Mode);
  Mode.Origin = Origin;
  FillSignalInfo(Mode.MonitorVideoSignalInfo, Width, Height, VSync, true);

  return Mode;
}

static IDDCX_TARGET_MODE CreateIddCxTargetMode(DWORD Width, DWORD Height, DWORD VSync) {
  IDDCX_TARGET_MODE Mode = {};

  Mode.Size = sizeof(Mode);
  FillSignalInfo(Mode.TargetVideoSignalInfo.targetVideoSignalInfo, Width, Height, VSync, false);

  return Mode;
}

#pragma endregion

extern "C" DRIVER_INITIALIZE DriverEntry;

EVT_WDF_DRIVER_DEVICE_ADD IddSampleDeviceAdd;
EVT_WDF_DEVICE_D0_ENTRY IddSampleDeviceD0Entry;
EVT_WDF_DRIVER_UNLOAD IddDriverUnload;

EVT_IDD_CX_ADAPTER_INIT_FINISHED IddSampleAdapterInitFinished;
EVT_IDD_CX_ADAPTER_COMMIT_MODES IddSampleAdapterCommitModes;

EVT_IDD_CX_PARSE_MONITOR_DESCRIPTION IddSampleParseMonitorDescription;
EVT_IDD_CX_MONITOR_GET_DEFAULT_DESCRIPTION_MODES IddSampleMonitorGetDefaultModes;
EVT_IDD_CX_MONITOR_QUERY_TARGET_MODES IddSampleMonitorQueryModes;

EVT_IDD_CX_MONITOR_ASSIGN_SWAPCHAIN IddSampleMonitorAssignSwapChain;
EVT_IDD_CX_MONITOR_UNASSIGN_SWAPCHAIN IddSampleMonitorUnassignSwapChain;

EVT_IDD_CX_DEVICE_IO_CONTROL IddSampleIoDeviceControl;

extern "C" BOOL WINAPI DllMain(
  _In_ HINSTANCE hInstance,
  _In_ UINT dwReason,
  _In_opt_ LPVOID lpReserved) {
  UNREFERENCED_PARAMETER(hInstance);
  UNREFERENCED_PARAMETER(lpReserved);
  UNREFERENCED_PARAMETER(dwReason);

  return TRUE;
}



_Use_decl_annotations_
extern "C" NTSTATUS DriverEntry(
  PDRIVER_OBJECT  pDriverObject,
  PUNICODE_STRING pRegistryPath
) {
  WDF_DRIVER_CONFIG Config;
  NTSTATUS Status;

  WDF_OBJECT_ATTRIBUTES Attributes;
  WDF_OBJECT_ATTRIBUTES_INIT(&Attributes);

  WDF_DRIVER_CONFIG_INIT(&Config,
    IddSampleDeviceAdd
  );
  Config.EvtDriverUnload = IddDriverUnload;

  Status = WdfDriverCreate(pDriverObject, pRegistryPath, &Attributes, &Config, WDF_NO_HANDLE);
  if (!NT_SUCCESS(Status)) {
    return Status;
  }

  return Status;
}

_Use_decl_annotations_
VOID IddDriverUnload(WDFDRIVER Driver) {
  UNREFERENCED_PARAMETER(Driver);
  return;
}

_Use_decl_annotations_
NTSTATUS IddSampleDeviceAdd(WDFDRIVER Driver, PWDFDEVICE_INIT pDeviceInit) {
  NTSTATUS Status = STATUS_SUCCESS;
  WDF_PNPPOWER_EVENT_CALLBACKS PnpPowerCallbacks;

  UNREFERENCED_PARAMETER(Driver);

  // Register for power callbacks - in this sample only power-on is needed
  WDF_PNPPOWER_EVENT_CALLBACKS_INIT(&PnpPowerCallbacks);
  PnpPowerCallbacks.EvtDeviceD0Entry = IddSampleDeviceD0Entry;
  WdfDeviceInitSetPnpPowerEventCallbacks(pDeviceInit, &PnpPowerCallbacks);

  IDD_CX_CLIENT_CONFIG IddConfig;
  IDD_CX_CLIENT_CONFIG_INIT(&IddConfig);

  // If the driver wishes to handle custom IoDeviceControl requests, it's necessary to use this callback since IddCx
  // redirects IoDeviceControl requests to an internal queue. This sample does not need this.
  IddConfig.EvtIddCxDeviceIoControl = IddSampleIoDeviceControl;

  IddConfig.EvtIddCxAdapterInitFinished = IddSampleAdapterInitFinished;

  IddConfig.EvtIddCxParseMonitorDescription = IddSampleParseMonitorDescription;
  IddConfig.EvtIddCxMonitorGetDefaultDescriptionModes = IddSampleMonitorGetDefaultModes;
  IddConfig.EvtIddCxMonitorQueryTargetModes = IddSampleMonitorQueryModes;
  IddConfig.EvtIddCxAdapterCommitModes = IddSampleAdapterCommitModes;
  IddConfig.EvtIddCxMonitorAssignSwapChain = IddSampleMonitorAssignSwapChain;
  IddConfig.EvtIddCxMonitorUnassignSwapChain = IddSampleMonitorUnassignSwapChain;

  Status = IddCxDeviceInitConfig(pDeviceInit, &IddConfig);
  if (!NT_SUCCESS(Status)) {
    return Status;
  }

  WDF_OBJECT_ATTRIBUTES Attr;
  WDF_OBJECT_ATTRIBUTES_INIT_CONTEXT_TYPE(&Attr, IndirectDeviceContextWrapper);
  Attr.EvtCleanupCallback = [](WDFOBJECT Object) {
    // Automatically cleanup the context when the WDF object is about to be deleted
    auto* pContext = WdfObjectGet_IndirectDeviceContextWrapper(Object);
    if (pContext) {
      pContext->Cleanup();
    }
  };

  WDFDEVICE Device = nullptr;
  Status = WdfDeviceCreate(&pDeviceInit, &Attr, &Device);
  if (!NT_SUCCESS(Status)) {
    return Status;
  }

  Status = IddCxDeviceInitialize(Device);

  // Create a new device context object and attach it to the WDF device object
  auto* pContext = WdfObjectGet_IndirectDeviceContextWrapper(Device);
  pContext->pContext = new IndirectDeviceContext(Device);

  return Status;
}

_Use_decl_annotations_
VOID IddSampleIoDeviceControl(_In_
  WDFDEVICE Device,
  _In_
  WDFREQUEST Request,
  _In_
  size_t OutputBufferLength,
  _In_
  size_t InputBufferLength,
  _In_
  ULONG IoControlCode) {
  NTSTATUS Status = STATUS_SUCCESS;

  auto* pContext = WdfObjectGet_IndirectDeviceContextWrapper(Device);

  PVOID  Buffer;
  size_t BufSize;

  switch (IoControlCode) {
  case IOCTL_CHANGER_IDD_PLUG_IN:
    Status = WdfRequestRetrieveInputBuffer(Request, sizeof(IoctlPlugIn), &Buffer, &BufSize);
    if (!NT_SUCCESS(Status)) {
      break;
    }
    auto* inputData = (IoctlPlugIn*)Buffer;
    pContext->pContext->CreateMonitor(inputData->id);
    break;
  }

  WdfRequestComplete(Request, Status);
}

_Use_decl_annotations_
NTSTATUS IddSampleParseMonitorDescription(
  const IDARG_IN_PARSEMONITORDESCRIPTION* pInArgs,
  IDARG_OUT_PARSEMONITORDESCRIPTION* pOutArgs
) {
  UNREFERENCED_PARAMETER(pInArgs);
  UNREFERENCED_PARAMETER(pOutArgs);

  // Do nothing as we do not use EDID in this driver.

  return STATUS_INVALID_PARAMETER;
}

_Use_decl_annotations_
NTSTATUS IddSampleDeviceD0Entry(WDFDEVICE Device, WDF_POWER_DEVICE_STATE PreviousState) {
  UNREFERENCED_PARAMETER(PreviousState);

  // This function is called by WDF to start the device in the fully-on power state.

  auto* pContext = WdfObjectGet_IndirectDeviceContextWrapper(Device);
  pContext->pContext->InitAdapter();

  return STATUS_SUCCESS;
}

#pragma region DDI Callbacks

_Use_decl_annotations_
NTSTATUS IddSampleAdapterInitFinished(IDDCX_ADAPTER AdapterObject, const IDARG_IN_ADAPTER_INIT_FINISHED* pInArgs) {
  // This is called when the OS has finished setting up the adapter for use by the IddCx driver. It's now possible
  // to report attached monitors.

  auto* pDeviceContextWrapper = WdfObjectGet_IndirectDeviceContextWrapper(AdapterObject);
  if (NT_SUCCESS(pInArgs->AdapterInitStatus)) {
    for (DWORD i = 0; i < IDD_SAMPLE_MONITOR_COUNT; i++) {
      pDeviceContextWrapper->pContext->CreateMonitor(i);
    }
  }

  return STATUS_SUCCESS;
}

_Use_decl_annotations_
NTSTATUS IddSampleAdapterCommitModes(IDDCX_ADAPTER AdapterObject, const IDARG_IN_COMMITMODES* pInArgs) {
  UNREFERENCED_PARAMETER(AdapterObject);

  for (UINT i = 0; i < pInArgs->PathCount; i++) {
    auto& path = pInArgs->pPaths[i];

    if ((path.Flags & IDDCX_PATH_FLAGS_ACTIVE) == 0) {
      // This path is inactive, ignore
      continue;
    }

    if ((path.Flags & IDDCX_PATH_FLAGS_CHANGED) == 0) {
      // This path has not changed
      continue;
    }

    auto& mode = path.TargetVideoSignalInfo;
    auto* pMonitorContextWrapper = WdfObjectGet_IndirectMonitorContextWrapper(path.MonitorObject);
    pMonitorContextWrapper->pContext->CommitModes(mode);
  }

  return STATUS_SUCCESS;
}

_Use_decl_annotations_
NTSTATUS IddSampleMonitorGetDefaultModes(IDDCX_MONITOR MonitorObject, const IDARG_IN_GETDEFAULTDESCRIPTIONMODES* pInArgs, IDARG_OUT_GETDEFAULTDESCRIPTIONMODES* pOutArgs) {
  UNREFERENCED_PARAMETER(MonitorObject);

  if (pInArgs->DefaultMonitorModeBufferInputCount == 0) {
    // The system asked for the size of this array
    pOutArgs->DefaultMonitorModeBufferOutputCount = ARRAYSIZE(s_DefaultModes);
  }
  else {
    UINT PreferredModeIndex = NO_PREFERRED_MODE;

    for (DWORD ModeIndex = 0; ModeIndex < ARRAYSIZE(s_DefaultModes); ModeIndex++) {
      auto mode = &s_DefaultModes[ModeIndex];

      pInArgs->pDefaultMonitorModes[ModeIndex] = CreateIddCxMonitorMode(
        mode->Width,
        mode->Height,
        mode->VSync,
        IDDCX_MONITOR_MODE_ORIGIN_DRIVER
      );

      if (mode->Width == 1920 && mode->Height == 1080 && mode->VSync == 60) {
        PreferredModeIndex = ModeIndex;
      }
    }

    pOutArgs->DefaultMonitorModeBufferOutputCount = ARRAYSIZE(s_DefaultModes);
    pOutArgs->PreferredMonitorModeIdx = PreferredModeIndex;
  }

  return STATUS_SUCCESS;
}

_Use_decl_annotations_
NTSTATUS IddSampleMonitorQueryModes(IDDCX_MONITOR MonitorObject, const IDARG_IN_QUERYTARGETMODES* pInArgs, IDARG_OUT_QUERYTARGETMODES* pOutArgs) {
  UNREFERENCED_PARAMETER(MonitorObject);

  vector<IDDCX_TARGET_MODE> TargetModes;

  // Create a set of modes supported for frame processing and scan-out. These are typically not based on the
  // monitor's descriptor and instead are based on the static processing capability of the device. The OS will
  // report the available set of modes for a given output as the intersection of monitor modes with target modes.

  //TargetModes.push_back(CreateIddCxTargetMode(3840, 2160, 60));
  //TargetModes.push_back(CreateIddCxTargetMode(2560, 1440, 144));
  //TargetModes.push_back(CreateIddCxTargetMode(2560, 1440, 90));
  //TargetModes.push_back(CreateIddCxTargetMode(2560, 1440, 60));
  //TargetModes.push_back(CreateIddCxTargetMode(1920, 1080, 144));
  //TargetModes.push_back(CreateIddCxTargetMode(1920, 1080, 90));
  //TargetModes.push_back(CreateIddCxTargetMode(1920, 1080, 60));
  //TargetModes.push_back(CreateIddCxTargetMode(1600, 900, 60));
  //TargetModes.push_back(CreateIddCxTargetMode(1024, 768, 75));
  //TargetModes.push_back(CreateIddCxTargetMode(1024, 768, 60));

  for (DWORD ModeIndex = 0; ModeIndex < ARRAYSIZE(s_DefaultModes); ModeIndex++) {
    TargetModes.push_back(CreateIddCxTargetMode(
      s_DefaultModes[ModeIndex].Width,
      s_DefaultModes[ModeIndex].Height,
      s_DefaultModes[ModeIndex].VSync
    ));
  }

  pOutArgs->TargetModeBufferOutputCount = (UINT)TargetModes.size();

  if (pInArgs->TargetModeBufferInputCount >= TargetModes.size()) {
    copy(TargetModes.begin(), TargetModes.end(), pInArgs->pTargetModes);
  }

  return STATUS_SUCCESS;
}

_Use_decl_annotations_
NTSTATUS IddSampleMonitorAssignSwapChain(IDDCX_MONITOR MonitorObject, const IDARG_IN_SETSWAPCHAIN* pInArgs) {
  auto* pMonitorContextWrapper = WdfObjectGet_IndirectMonitorContextWrapper(MonitorObject);
  pMonitorContextWrapper->pContext->AssignSwapChain(pInArgs->hSwapChain, pInArgs->RenderAdapterLuid, pInArgs->hNextSurfaceAvailable);
  return STATUS_SUCCESS;
}

_Use_decl_annotations_
NTSTATUS IddSampleMonitorUnassignSwapChain(IDDCX_MONITOR MonitorObject) {
  auto* pMonitorContextWrapper = WdfObjectGet_IndirectMonitorContextWrapper(MonitorObject);
  pMonitorContextWrapper->pContext->UnassignSwapChain();
  return STATUS_SUCCESS;
}

#pragma endregion
