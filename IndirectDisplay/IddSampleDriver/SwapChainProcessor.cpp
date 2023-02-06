#include "Driver.h"
#include "Driver.tmh"

using namespace Microsoft::IndirectDisp;
using namespace Microsoft::WRL;

SwapChainProcessor::SwapChainProcessor(
  IDDCX_SWAPCHAIN hSwapChain, std::shared_ptr<Direct3DDevice> Device, HANDLE NewFrameEvent, std::shared_ptr<MonitorClient> MonitorClient
)
  : m_hSwapChain(hSwapChain), m_Device(Device), m_hAvailableBufferEvent(NewFrameEvent), m_RustMonitor(MonitorClient) {
  m_hTerminateEvent.Attach(CreateEvent(nullptr, FALSE, FALSE, nullptr));

  // Immediately create and run the swap-chain processing thread, passing 'this' as the thread parameter
  m_hThread.Attach(CreateThread(nullptr, 0, RunThread, this, 0, nullptr));
}

SwapChainProcessor::~SwapChainProcessor() {
  // Alert the swap-chain processing thread to terminate
  SetEvent(m_hTerminateEvent.Get());

  if (m_hThread.Get()) {
    // Wait for the thread to terminate
    WaitForSingleObject(m_hThread.Get(), INFINITE);
  }
}

DWORD CALLBACK SwapChainProcessor::RunThread(LPVOID Argument) {
  reinterpret_cast<SwapChainProcessor*>(Argument)->Run();
  return 0;
}

void SwapChainProcessor::Run() {
  // For improved performance, make use of the Multimedia Class Scheduler Service, which will intelligently
  // prioritize this thread for improved throughput in high CPU-load scenarios.
  DWORD AvTask = 0;
  HANDLE AvTaskHandle = AvSetMmThreadCharacteristicsW(L"Distribution", &AvTask);

  RunCore();

  // Always delete the swap-chain object when swap-chain processing loop terminates in order to kick the system to
  // provide a new swap-chain if necessary.
  WdfObjectDelete((WDFOBJECT)m_hSwapChain);
  m_hSwapChain = nullptr;

  AvRevertMmThreadCharacteristics(AvTaskHandle);
}

void SwapChainProcessor::RunCore() {
  // Get the DXGI device interface
  ComPtr<IDXGIDevice> DxgiDevice;
  HRESULT hr = m_Device->Device.As(&DxgiDevice);
  if (FAILED(hr)) {
    return;
  }

  IDARG_IN_SWAPCHAINSETDEVICE SetDevice = {};
  SetDevice.pDevice = DxgiDevice.Get();

  hr = IddCxSwapChainSetDevice(m_hSwapChain, &SetDevice);
  if (FAILED(hr)) {
    return;
  }

  D3D11_TEXTURE2D_DESC cpuImageDesc = { 0 };
  cpuImageDesc.Width = 1920;
  cpuImageDesc.Height = 1080;
  cpuImageDesc.MipLevels = 1;
  cpuImageDesc.ArraySize = 1;
  cpuImageDesc.Format = DXGI_FORMAT_B8G8R8A8_UNORM;
  cpuImageDesc.SampleDesc = { 1, 0 };
  cpuImageDesc.Usage = D3D11_USAGE_STAGING;
  cpuImageDesc.BindFlags = 0;
  cpuImageDesc.CPUAccessFlags = D3D11_CPU_ACCESS_READ;
  cpuImageDesc.MiscFlags = 0;
  ComPtr<ID3D11Texture2D> cpuImage = NULL;


  // Acquire and release buffers in a loop
  for (;;) {
    ComPtr<IDXGIResource> AcquiredBuffer;

    // Ask for the next buffer from the producer
    IDARG_OUT_RELEASEANDACQUIREBUFFER Buffer = {};
    hr = IddCxSwapChainReleaseAndAcquireBuffer(m_hSwapChain, &Buffer);

    // AcquireBuffer immediately returns STATUS_PENDING if no buffer is yet available
    if (hr == E_PENDING) {
      // We must wait for a new buffer
      HANDLE WaitHandles[] =
      {
          m_hAvailableBufferEvent,
          m_hTerminateEvent.Get()
      };
      DWORD WaitResult = WaitForMultipleObjects(ARRAYSIZE(WaitHandles), WaitHandles, FALSE, 16);
      if (WaitResult == WAIT_OBJECT_0 || WaitResult == WAIT_TIMEOUT) {
        // We have a new buffer, so try the AcquireBuffer again
        continue;
      }
      else if (WaitResult == WAIT_OBJECT_0 + 1) {
        // We need to terminate
        break;
      }
      else {
        // The wait was cancelled or something unexpected happened
        hr = HRESULT_FROM_WIN32(WaitResult);
        break;
      }
    }
    else if (SUCCEEDED(hr)) {
      // We have new frame to process, the surface has a reference on it that the driver has to release
      AcquiredBuffer.Attach(Buffer.MetaData.pSurface);

      ComPtr<ID3D11Texture2D> gpuImage;

      if (Buffer.MetaData.DirtyRectCount == 0 && Buffer.MetaData.MoveRegionCount == 0) {
        goto next;
      }

      hr = AcquiredBuffer->QueryInterface(_uuidof(ID3D11Texture2D), &gpuImage);
      if (FAILED(hr)) {
        goto next;
      }

      D3D11_TEXTURE2D_DESC gpuImageDesc;
      gpuImage->GetDesc(&gpuImageDesc);

      if (cpuImage == NULL || gpuImageDesc.Height != cpuImageDesc.Height || gpuImageDesc.Width != cpuImageDesc.Width) {
        cpuImageDesc.Height = gpuImageDesc.Height;
        cpuImageDesc.Width = gpuImageDesc.Width;

        hr = m_Device->Device->CreateTexture2D(&cpuImageDesc, nullptr, &cpuImage);
        if (FAILED(hr)) {
          goto next;
        }
      }

      m_Device->DeviceContext->CopyResource(cpuImage.Get(), gpuImage.Get());

      D3D11_MAPPED_SUBRESOURCE mappedCpuImage;
      hr = m_Device->DeviceContext->Map(cpuImage.Get(), 0, D3D11_MAP_READ, 0, &mappedCpuImage);
      if (FAILED(hr)) {
        goto next;
      }

      m_RustMonitor->SendFrame(
        static_cast<uint8_t*>(mappedCpuImage.pData), cpuImageDesc.Width, cpuImageDesc.Height, mappedCpuImage.RowPitch
      );

      m_Device->DeviceContext->Unmap(cpuImage.Get(), 0);

      // We have finished processing this frame hence we release the reference on it.
      // If the driver forgets to release the reference to the surface, it will be leaked which results in the
      // surfaces being left around after swapchain is destroyed.
      // NOTE: Although in this sample we release reference to the surface here; the driver still
      // owns the Buffer.MetaData.pSurface surface until IddCxSwapChainReleaseAndAcquireBuffer returns
      // S_OK and gives us a new frame, a driver may want to use the surface in future to re-encode the desktop 
      // for better quality if there is no new frame for a while
    next:
      AcquiredBuffer.Reset();

      // Indicate to OS that we have finished inital processing of the frame, it is a hint that
      // OS could start preparing another frame
      hr = IddCxSwapChainFinishedProcessingFrame(m_hSwapChain);
      if (FAILED(hr)) {
        break;
      }

      // ==============================
      // TODO: Report frame statistics once the asynchronous encode/send work is completed
      //
      // Drivers should report information about sub-frame timings, like encode time, send time, etc.
      // ==============================
      // IddCxSwapChainReportFrameStatistics(m_hSwapChain, ...);
    }
    else {
      //ERR("The swap-chain was likely abandoned (e.g. DXGI_ERROR_ACCESS_LOST)");
      break;
    }
  }
}