#pragma once

#define NOMINMAX
#include <windows.h>
#include <bugcodes.h>
#include <wudfwdm.h>
#include <wdf.h>
#include <iddcx.h>

#include <dxgi1_5.h>
#include <d3d11_2.h>
#include <avrt.h>
#include <wrl.h>

#include <memory>
#include <vector>

#include "Trace.h"

//extern "C" {
//  typedef struct vd_monitor_t vd_monitor_t;
//
//  void vd_init();
//  void vd_log(const char* msg);
//  void vd_log_debug(const char* msg);
//
//  vd_monitor_t* vd_monitor_new(uint32_t index);
//  void vd_monitor_free(vd_monitor_t* monitor);
//  void vd_monitor_send_frame(vd_monitor_t* monitor, const uint8_t* buffer, size_t len);
//  void vd_monitor_configure(vd_monitor_t* monitor, uint32_t width, uint32_t height, uint32_t framerate);
//}

namespace Microsoft {
  namespace WRL {
    namespace Wrappers {
      // Adds a wrapper for thread handles to the existing set of WRL handle wrapper classes
      typedef HandleT<HandleTraits::HANDLENullTraits> Thread;
    }
  }
}

namespace Microsoft {
  namespace IndirectDisp {
    /// <summary>
    /// Manages the creation and lifetime of a Direct3D render device.
    /// </summary>
    struct IndirectSampleMonitor {
      static constexpr size_t szEdidBlock = 128;
      static constexpr size_t szModeList = 3;

      const BYTE pEdidBlock[szEdidBlock];
      const struct SampleMonitorMode {
        DWORD Width;
        DWORD Height;
        DWORD VSync;
      } pModeList[szModeList];
      const DWORD ulPreferredModeIdx;
    };

    /// <summary>
    /// Manages the creation and lifetime of a Direct3D render device.
    /// </summary>
    struct Direct3DDevice {
      Direct3DDevice(LUID AdapterLuid);
      Direct3DDevice();
      HRESULT Init();

      LUID AdapterLuid;
      Microsoft::WRL::ComPtr<IDXGIFactory5> DxgiFactory;
      Microsoft::WRL::ComPtr<IDXGIAdapter1> Adapter;
      Microsoft::WRL::ComPtr<ID3D11Device> Device;
      Microsoft::WRL::ComPtr<ID3D11DeviceContext> DeviceContext;
    };

    /// <summary>
    /// Represents a monitor in the Rust side.
    /// </summary>
    class MonitorClient {
    public:
      MonitorClient(UINT ConnectorIndex);
      ~MonitorClient();

      void Configure(uint32_t width, uint32_t height, uint32_t framerate);
      void SendFrame(const uint8_t* buffer, size_t buffer_len);

    private:
      Microsoft::WRL::Wrappers::Mutex m_FrameBufferMutex;
      Microsoft::WRL::Wrappers::Event m_NewFrameEvent;
      Microsoft::WRL::Wrappers::Event m_ConfigureEvent;

      uint8_t* m_FrameBuffer = nullptr;
      HANDLE m_FrameBufferMapping = nullptr;
    };

    /// <summary>
    /// Manages a thread that consumes buffers from an indirect display swap-chain object.
    /// </summary>
    class SwapChainProcessor {
    public:
      SwapChainProcessor(
        IDDCX_SWAPCHAIN hSwapChain, std::shared_ptr<Direct3DDevice> Device, HANDLE NewFrameEvent, std::shared_ptr<MonitorClient> MonitorClient
      );
      ~SwapChainProcessor();

    private:
      static DWORD CALLBACK RunThread(LPVOID Argument);

      void Run();
      void RunCore();

      IDDCX_SWAPCHAIN m_hSwapChain;

      std::shared_ptr<Direct3DDevice> m_Device;

      HANDLE m_hAvailableBufferEvent;

      std::shared_ptr<MonitorClient> m_RustMonitor;

      Microsoft::WRL::Wrappers::Thread m_hThread;
      Microsoft::WRL::Wrappers::Event m_hTerminateEvent;
    };

    /// <summary>
    /// Provides a sample implementation of an indirect display driver.
    /// </summary>
    class IndirectDeviceContext {
    public:
      IndirectDeviceContext(_In_ WDFDEVICE WdfDevice);
      virtual ~IndirectDeviceContext();

      void InitAdapter();
      void CreateMonitor(UINT ConnectorIndex);

    protected:
      WDFDEVICE m_WdfDevice;
      IDDCX_ADAPTER m_Adapter;
    };

    /// <summary>
    /// Represents a single monitor in this driver.
    /// </summary>
    class IndirectMonitorContext {
    public:
      IndirectMonitorContext(_In_ IDDCX_MONITOR Monitor, UINT ConnectorIndex);
      virtual ~IndirectMonitorContext();

      void AssignSwapChain(IDDCX_SWAPCHAIN SwapChain, LUID RenderAdapter, HANDLE NewFrameEvent);
      void UnassignSwapChain();

      void Configure(DISPLAYCONFIG_VIDEO_SIGNAL_INFO& mode);

    private:
      IDDCX_MONITOR m_Monitor;
      std::shared_ptr<MonitorClient> m_RustMonitor;
      std::unique_ptr<SwapChainProcessor> m_ProcessingThread;
    };
  }
}

struct IndirectDeviceContextWrapper {
  Microsoft::IndirectDisp::IndirectDeviceContext* pContext;

  void Cleanup() {
    delete pContext;
    pContext = nullptr;
  }
};

struct IndirectMonitorContextWrapper {
  Microsoft::IndirectDisp::IndirectMonitorContext* pContext;

  void Cleanup() {
    delete pContext;
    pContext = nullptr;
  }
};

// This macro creates the methods for accessing an IndirectDeviceContextWrapper as a context for a WDF object
WDF_DECLARE_CONTEXT_TYPE(IndirectDeviceContextWrapper);

WDF_DECLARE_CONTEXT_TYPE(IndirectMonitorContextWrapper);

static constexpr DWORD IDD_SAMPLE_MONITOR_COUNT = 1; // If monitor count > ARRAYSIZE(s_SampleMonitors), we create edid-less monitors

