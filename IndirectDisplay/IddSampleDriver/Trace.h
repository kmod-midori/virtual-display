#pragma once

/*++

Module Name:

    Internal.h

Abstract:

    This module contains the local type definitions for the
    driver.

Environment:

    Windows User-Mode Driver Framework 2

--*/

//
// Define the tracing flags.
//
// Tracing GUID - b254994f-46e6-4718-80a0-0a3aa50d6ce4
//

#define WPP_CONTROL_GUIDS                                              \
    WPP_DEFINE_CONTROL_GUID(                                           \
        MyDriver1TraceGuid, (b254994f,46e6,4718,80a0,0a3aa50d6ce4),                  \
                                                                       \
        WPP_DEFINE_BIT(verbose)                              \
        WPP_DEFINE_BIT(information)                                   \
        WPP_DEFINE_BIT(warning)                                   \
        WPP_DEFINE_BIT(error)                                    \
        )                             

#define WPP_FLAG_LEVEL_LOGGER(flag, level)                             \
    WPP_LEVEL_LOGGER(flag)

#define WPP_FLAG_LEVEL_ENABLED(flag, level)                            \
    (WPP_LEVEL_ENABLED(flag) &&                                        \
     WPP_CONTROL(WPP_BIT_ ## flag).Level >= level)

#define WPP_LEVEL_FLAGS_LOGGER(lvl,flags)                              \
           WPP_LEVEL_LOGGER(flags)
               
#define WPP_LEVEL_FLAGS_ENABLED(lvl, flags)                            \
           (WPP_LEVEL_ENABLED(flags) && WPP_CONTROL(WPP_BIT_ ## flags).Level >= lvl)

//
// This comment block is scanned by the trace preprocessor to define our
// Trace function.
//
// begin_wpp config
// FUNC ERR{LEVEL=TRACE_LEVEL_ERROR,FLAGS=error}(MSG,...);
// FUNC WARN{LEVEL=TRACE_LEVEL_WARNING,FLAGS=warning}(MSG,...);
// FUNC INFO{LEVEL=TRACE_LEVEL_INFORMATION,FLAGS=information}(MSG,...);
// FUNC DBGPRINT{LEVEL=TRACE_LEVEL_INFORMATION,FLAGS=verbose}(MSG,...);
// USEPREFIX(ERR, "%!STDPREFIX! [%!FUNC!:%!LINE!] [ERR] \t");
// USEPREFIX(WARN, "%!STDPREFIX! [%!FUNC!:%!LINE!] [WARN] \t");
// USEPREFIX(DBGPRINT, "%!STDPREFIX! [%!FUNC!:%!LINE!] [DBG] \t");
// USEPREFIX(INFO, "%!STDPREFIX! [%!FUNC!:%!LINE!] [INFO] \t");
// end_wpp

//
//
// Driver specific #defines
//

// TODO: Use a unique driver tracing ID here,
// see https://docs.microsoft.com/en-us/windows-hardware/drivers/devtest/adding-wpp-software-tracing-to-a-windows-driver
#define MYDRIVER_TRACING_ID L"Microsoft\\UMDF2.25\\IddSampleDriver v1.0"
