// Verilated -*- C++ -*-
// DESCRIPTION: Verilator output: Design internal header
// See Voe.h for the primary calling header

#ifndef VERILATED_VOE___024ROOT_H_
#define VERILATED_VOE___024ROOT_H_  // guard

#include "verilated.h"
#include "verilated_timing.h"


class Voe__Syms;

class alignas(VL_CACHE_LINE_BYTES) Voe___024root final {
  public:

    // DESIGN SPECIFIC STATE
    CData/*0:0*/ oe__DOT__clk;
    CData/*0:0*/ oe__DOT__sda_oe;
    CData/*0:0*/ __Vtrigprevexpr___TOP__oe__DOT__clk__0;
    CData/*0:0*/ __VactPhaseResult;
    CData/*0:0*/ __VinactPhaseResult;
    CData/*0:0*/ __VnbaPhaseResult;
    IData/*31:0*/ __VactIterCount;
    IData/*31:0*/ __VinactIterCount;
    IData/*31:0*/ __Vi;
    VlUnpacked<QData/*63:0*/, 1> __VactTriggered;
    VlUnpacked<QData/*63:0*/, 1> __VactTriggeredAcc;
    VlUnpacked<QData/*63:0*/, 1> __VnbaTriggered;
    VlDelayScheduler __VdlySched;

    // INTERNAL VARIABLES
    Voe__Syms* vlSymsp;
    const char* vlNamep;

    // CONSTRUCTORS
    Voe___024root(Voe__Syms* symsp, const char* namep);
    ~Voe___024root();
    VL_UNCOPYABLE(Voe___024root);

    // INTERNAL METHODS
    void __Vconfigure(bool first);
};


#endif  // guard
