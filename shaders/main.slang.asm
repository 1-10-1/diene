; SPIR-V
; Version: 1.5
; Generator: Khronos Slang Compiler; 0
; Bound: 177
; Schema: 0
OpCapability DrawParameters
OpCapability Shader
OpExtension "SPV_KHR_non_semantic_info"
%1 = OpExtInstImport "NonSemantic.Shader.DebugInfo.100"
OpMemoryModel Logical GLSL450
OpEntryPoint Fragment %fragMain "fragMain" %entryPointParam_fragMain %inVert_color
OpEntryPoint Vertex %vertMain "vertMain" %entryPointParam_vertMain_color %gl_Position %gl_VertexIndex %9
OpExecutionMode %fragMain OriginUpperLeft

; Debug Information
%10 = OpString "static float2 positions[3] = float2[](
    float2(0.0, -0.5),
    float2(0.5, 0.5),
    float2(-0.5, 0.5)
);

static float3 colors[3] = float3[](
    float3(0.0, 0.0, 0.0),
    float3(0.0, 1.0, 0.0),
    float3(0.0, 0.0, 1.0)
);

struct VertexOutput {
    float3 color;
    float4 sv_position : SV_Position;
};

[shader(\"vertex\")]
VertexOutput vertMain(uint vid : SV_VertexID) {
    VertexOutput output;
    output.sv_position = float4(positions[vid], 0.0, 1.0);
    output.color = colors[vid];
    return output;
}

[shader(\"fragment\")]
float4 fragMain(VertexOutput inVert) : SV_Target
{
    float3 color = inVert.color;
    return float4(color, 1.0);
}
"
%11 = OpString "/home/aether/dev/diene/shaders/main.slang"
OpSource Slang 1
%12 = OpString "float"
%13 = OpString "color"
%14 = OpString "sv_position"
%15 = OpString "VertexOutput"
%16 = OpString "fragMain"
%17 = OpString "slangc"
%18 = OpString "-target spirv  -I \"shaders\" -I \"shaders\" -O3 -matrix-layout-column-major -stage pixel -entry fragMain -g2"
%19 = OpString "positions"
%20 = OpString "colors"
%21 = OpString "inVert"
%22 = OpString "inVert.color"
%23 = OpString "entryPointParam_fragMain"
%24 = OpString "uint"
%25 = OpString "vertMain"
%26 = OpString "-target spirv  -I \"shaders\" -I \"shaders\" -O3 -matrix-layout-column-major -stage vertex -entry vertMain -g2"
%27 = OpString "vid"
%28 = OpString "output"
%29 = OpString "entryPointParam_vertMain.color"
OpName %positions "positions"                       ; id %30
OpName %colors "colors"                             ; id %31
OpName %inVert "inVert"                             ; id %32
OpName %inVert_color "inVert.color"                 ; id %4
OpName %color "color"                               ; id %33
OpName %entryPointParam_fragMain "entryPointParam_fragMain"     ; id %3
OpName %fragMain "fragMain"                                     ; id %2
OpName %vid "vid"                                               ; id %34
OpName %output "output"                                         ; id %35
OpName %entryPointParam_vertMain_color "entryPointParam_vertMain.color"     ; id %6
OpName %vertMain "vertMain"                                                 ; id %5

; Annotations
OpDecorate %inVert_color Location 0
OpDecorate %entryPointParam_fragMain Location 0
OpDecorate %9 BuiltIn BaseVertex
OpDecorate %gl_VertexIndex BuiltIn VertexIndex
OpDecorate %entryPointParam_vertMain_color Location 0
OpDecorate %gl_Position BuiltIn Position

; Types, variables and constants
%void = OpTypeVoid
%uint = OpTypeInt 32 0
%uint_11 = OpConstant %uint 11
%uint_5 = OpConstant %uint 5
%uint_100 = OpConstant %uint 100
%88 = OpTypeFunction %void
%float = OpTypeFloat 32
%v3float = OpTypeVector %float 3
%v4float = OpTypeVector %float 4
%_ptr_Function_v3float = OpTypePointer Function %v3float
%uint_32 = OpConstant %uint 32
%uint_3 = OpConstant %uint 3
%uint_131072 = OpConstant %uint 131072
%uint_4 = OpConstant %uint 4
%uint_14 = OpConstant %uint 14
%uint_12 = OpConstant %uint 12
%uint_0 = OpConstant %uint 0
%uint_96 = OpConstant %uint 96
%uint_15 = OpConstant %uint 15
%uint_128 = OpConstant %uint 128
%uint_1 = OpConstant %uint 1
%uint_13 = OpConstant %uint 13
%uint_8 = OpConstant %uint 8
%uint_224 = OpConstant %uint 224
%uint_27 = OpConstant %uint 27
%v2float = OpTypeVector %float 2
%int = OpTypeInt 32 1
%int_3 = OpConstant %int 3
%_arr_v2float_int_3 = OpTypeArray %v2float %int_3
%float_0 = OpConstant %float 0
%float_n0_5 = OpConstant %float -0.5
%99 = OpConstantComposite %v2float %float_0 %float_n0_5
%float_0_5 = OpConstant %float 0.5
%101 = OpConstantComposite %v2float %float_0_5 %float_0_5
%102 = OpConstantComposite %v2float %float_n0_5 %float_0_5
%103 = OpConstantComposite %_arr_v2float_int_3 %99 %101 %102
%uint_2 = OpConstant %uint 2
%_arr_v3float_int_3 = OpTypeArray %v3float %int_3
%105 = OpConstantComposite %v3float %float_0 %float_0 %float_0
%float_1 = OpConstant %float 1
%107 = OpConstantComposite %v3float %float_0 %float_1 %float_0
%108 = OpConstantComposite %v3float %float_0 %float_0 %float_1
%109 = OpConstantComposite %_arr_v3float_int_3 %105 %107 %108
%uint_7 = OpConstant %uint 7
%_ptr_Input_v3float = OpTypePointer Input %v3float
%int_0 = OpConstant %int 0
%int_1 = OpConstant %int 1
%uint_29 = OpConstant %uint 29
%uint_6 = OpConstant %uint 6
%uint_30 = OpConstant %uint 30
%_ptr_Output_v4float = OpTypePointer Output %v4float
%uint_19 = OpConstant %uint 19
%_ptr_Input_int = OpTypePointer Input %int
%uint_20 = OpConstant %uint 20
%uint_18 = OpConstant %uint 18
%uint_21 = OpConstant %uint 21
%uint_22 = OpConstant %uint 22
%uint_23 = OpConstant %uint 23
%_ptr_Output_v3float = OpTypePointer Output %v3float
%inVert_color = OpVariable %_ptr_Input_v3float Input    ; Location 0
%entryPointParam_fragMain = OpVariable %_ptr_Output_v4float Output  ; Location 0
%9 = OpVariable %_ptr_Input_int Input                               ; BuiltIn BaseVertex
%gl_VertexIndex = OpVariable %_ptr_Input_int Input                  ; BuiltIn VertexIndex
%entryPointParam_vertMain_color = OpVariable %_ptr_Output_v3float Output    ; Location 0
%gl_Position = OpVariable %_ptr_Output_v4float Output                       ; BuiltIn Position
%_ptr_Function__arr_v2float_int_3 = OpTypePointer Function %_arr_v2float_int_3
%_ptr_Function__arr_v3float_int_3 = OpTypePointer Function %_arr_v3float_int_3
%_ptr_Function_v2float = OpTypePointer Function %v2float
%123 = OpUndef %v3float
%124 = OpUndef %v4float
%125 = OpUndef %uint
%37 = OpExtInst %void %1 DebugInfoNone
%38 = OpExtInst %void %1 DebugExpression
%39 = OpExtInst %void %1 DebugSource %11 %10
%40 = OpExtInst %void %1 DebugCompilationUnit %uint_100 %uint_5 %39 %uint_11
%44 = OpExtInst %void %1 DebugTypeBasic %12 %uint_32 %uint_3 %uint_131072
%48 = OpExtInst %void %1 DebugTypeVector %44 %uint_4
%50 = OpExtInst %void %1 DebugTypeVector %44 %uint_3
%51 = OpExtInst %void %1 DebugTypeMember %13 %50 %39 %uint_14 %uint_12 %uint_0 %uint_96 %uint_0
%56 = OpExtInst %void %1 DebugTypeMember %14 %48 %39 %uint_15 %uint_12 %uint_96 %uint_128 %uint_0
%59 = OpExtInst %void %1 DebugTypeComposite %15 %uint_1 %39 %uint_13 %uint_8 %40 %15 %uint_224 %uint_131072 %51 %56
%64 = OpExtInst %void %1 DebugTypeFunction %uint_0 %48 %59
%65 = OpExtInst %void %1 DebugFunction %16 %64 %39 %uint_27 %uint_8 %40 %16 %uint_0 %uint_27
%67 = OpExtInst %void %1 DebugEntryPoint %65 %40 %17 %18
%68 = OpExtInst %void %1 DebugTypeVector %44 %uint_2
%70 = OpExtInst %void %1 DebugTypeArray %68 %uint_3
%71 = OpExtInst %void %1 DebugTypeArray %50 %uint_3
%inVert = OpExtInst %void %1 DebugLocalVariable %21 %59 %39 %uint_27 %uint_8 %65 %uint_0 %uint_1
%color = OpExtInst %void %1 DebugLocalVariable %13 %50 %39 %uint_29 %uint_12 %65 %uint_0
%73 = OpExtInst %void %1 DebugTypeBasic %24 %uint_32 %uint_6 %uint_131072
%75 = OpExtInst %void %1 DebugTypeFunction %uint_0 %59 %73
%76 = OpExtInst %void %1 DebugFunction %25 %75 %39 %uint_19 %uint_14 %40 %25 %uint_0 %uint_19
%78 = OpExtInst %void %1 DebugEntryPoint %76 %40 %17 %26
%vid = OpExtInst %void %1 DebugLocalVariable %27 %73 %39 %uint_19 %uint_14 %76 %uint_0 %uint_1
%output = OpExtInst %void %1 DebugLocalVariable %28 %59 %39 %uint_20 %uint_18 %76 %uint_0
%81 = OpExtInst %void %1 DebugLocalVariable %19 %70 %39 %uint_1 %uint_15 %40 %uint_0
%82 = OpExtInst %void %1 DebugLocalVariable %20 %71 %39 %uint_7 %uint_15 %40 %uint_0
%84 = OpExtInst %void %1 DebugGlobalVariable %22 %50 %39 %uint_0 %uint_0 %40 %22 %inVert_color %uint_0
%85 = OpExtInst %void %1 DebugGlobalVariable %23 %48 %39 %uint_0 %uint_0 %40 %23 %entryPointParam_fragMain %uint_0
%86 = OpExtInst %void %1 DebugGlobalVariable %29 %50 %39 %uint_0 %uint_0 %40 %29 %entryPointParam_vertMain_color %uint_0

; Function fragMain
%fragMain = OpFunction %void None %88
%126 = OpLabel
%127 = OpExtInst %void %1 DebugFunctionDefinition %65 %fragMain
%173 = OpExtInst %void %1 DebugScope %65
%128 = OpExtInst %void %1 DebugLine %39 %uint_27 %uint_27 %uint_8 %uint_8
%129 = OpExtInst %void %1 DebugValue %inVert %123 %38 %int_0
%131 = OpExtInst %void %1 DebugValue %inVert %124 %38 %int_1
%132 = OpExtInst %void %1 DebugLine %39 %uint_29 %uint_29 %uint_5 %uint_6
%133 = OpLoad %v3float %inVert_color
%135 = OpExtInst %void %1 DebugValue %color %133 %38
%136 = OpExtInst %void %1 DebugLine %39 %uint_30 %uint_30 %uint_5 %uint_6
%137 = OpCompositeConstruct %v4float %133 %float_1
OpStore %entryPointParam_fragMain %137
OpReturn
%174 = OpExtInst %void %1 DebugNoScope
OpFunctionEnd

; Function vertMain
%vertMain = OpFunction %void None %88
%140 = OpLabel
%colors = OpVariable %_ptr_Function__arr_v3float_int_3 Function
%positions = OpVariable %_ptr_Function__arr_v2float_int_3 Function
%141 = OpExtInst %void %1 DebugDeclare %82 %colors %38
%142 = OpExtInst %void %1 DebugDeclare %81 %positions %38
%143 = OpExtInst %void %1 DebugFunctionDefinition %76 %vertMain
%175 = OpExtInst %void %1 DebugScope %76
%144 = OpExtInst %void %1 DebugLine %39 %uint_19 %uint_19 %uint_14 %uint_14
OpStore %positions %103
OpStore %colors %109
%147 = OpExtInst %void %1 DebugValue %vid %125 %38
%148 = OpExtInst %void %1 DebugLine %39 %uint_21 %uint_21 %uint_5 %uint_6
%149 = OpLoad %int %9
%151 = OpLoad %int %gl_VertexIndex
%153 = OpISub %int %151 %149
%155 = OpBitcast %uint %153
%157 = OpAccessChain %_ptr_Function_v2float %positions %155
%159 = OpLoad %v2float %157
%161 = OpCompositeConstruct %v4float %159 %float_0 %float_1
%163 = OpExtInst %void %1 DebugValue %output %161 %38 %int_1
%164 = OpExtInst %void %1 DebugLine %39 %uint_22 %uint_22 %uint_5 %uint_6
%165 = OpAccessChain %_ptr_Function_v3float %colors %155
%167 = OpLoad %v3float %165
%169 = OpExtInst %void %1 DebugValue %output %167 %38 %int_0
%170 = OpExtInst %void %1 DebugLine %39 %uint_23 %uint_23 %uint_5 %uint_6
OpStore %entryPointParam_vertMain_color %167
OpStore %gl_Position %161
OpReturn
%176 = OpExtInst %void %1 DebugNoScope
OpFunctionEnd
