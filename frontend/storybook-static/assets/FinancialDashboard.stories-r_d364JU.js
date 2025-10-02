import{j as f}from"./jsx-runtime-LnmZrwv1.js";import{d as xt,i as ot,c as me,a as Ct,b as wt,e as _t,_ as q,f as h,g as at,s as nt,u as St,h as Tt,j as Et,k as Rt,r as Pt,l as $t,T as st,m as Q,n as Ce,o as Mt}from"./DefaultPropsProvider-y_IJv_Iv.js";import{r as L,b as V,g as Ft}from"./index-BdcX6MmR.js";import{c as H,E as Ot}from"./ErrorBoundary-d_UsmuKi.js";import{Q as It,a as jt}from"./QueryClientProvider-BD1Vikve.js";import"./index-Bnmt0x4K.js";const Nt=Object.freeze(Object.defineProperty({__proto__:null,default:xt,isPlainObject:ot},Symbol.toStringTag,{value:"Module"})),Dt=Object.freeze(Object.defineProperty({__proto__:null,default:me},Symbol.toStringTag,{value:"Module"})),At=Object.freeze(Object.defineProperty({__proto__:null,default:Ct,private_createBreakpoints:wt,unstable_applyStyles:_t},Symbol.toStringTag,{value:"Module"})),qt=["sx"],Wt=e=>{var t,r;const a={systemProps:{},otherProps:{}},i=(t=e==null||(r=e.theme)==null?void 0:r.unstable_sxConfig)!=null?t:at;return Object.keys(e).forEach(n=>{i[n]?a.systemProps[n]=e[n]:a.otherProps[n]=e[n]}),a};function he(e){const{sx:t}=e,r=q(e,qt),{systemProps:a,otherProps:i}=Wt(r);let n;return Array.isArray(t)?n=[a,...t]:typeof t=="function"?n=(...d)=>{const l=t(...d);return ot(l)?h({},a,l):a}:n=h({},a,t),h({},i,{sx:n})}const kt=Object.freeze(Object.defineProperty({__proto__:null,default:nt,extendSxProp:he,unstable_createStyleFunctionSx:St,unstable_defaultSxConfig:at},Symbol.toStringTag,{value:"Module"})),we=e=>e,Bt=()=>{let e=we;return{configure(t){e=t},generate(t){return e(t)},reset(){e=we}}},it=Bt(),Ut=["className","component"];function Lt(e={}){const{themeId:t,defaultTheme:r,defaultClassName:a="MuiBox-root",generateClassName:i}=e,n=Tt("div",{shouldForwardProp:l=>l!=="theme"&&l!=="sx"&&l!=="as"})(nt);return L.forwardRef(function(m,_){const g=Et(r),w=he(m),{className:R,component:$="div"}=w,u=q(w,Ut);return f.jsx(n,h({as:$,ref:_,className:H(R,i?i(a):a),theme:t&&g[t]||g},u))})}const Vt={active:"active",checked:"checked",completed:"completed",disabled:"disabled",error:"error",expanded:"expanded",focused:"focused",focusVisible:"focusVisible",open:"open",readOnly:"readOnly",required:"required",selected:"selected"};function Y(e,t,r="Mui"){const a=Vt[t];return a?`${r}-${a}`:`${it.generate(e)}-${t}`}function z(e,t,r="Mui"){const a={};return t.forEach(i=>{a[i]=Y(e,i,r)}),a}var de={exports:{}},c={};/**
 * @license React
 * react-is.production.js
 *
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */var _e;function Ht(){if(_e)return c;_e=1;var e=Symbol.for("react.transitional.element"),t=Symbol.for("react.portal"),r=Symbol.for("react.fragment"),a=Symbol.for("react.strict_mode"),i=Symbol.for("react.profiler"),n=Symbol.for("react.consumer"),d=Symbol.for("react.context"),l=Symbol.for("react.forward_ref"),m=Symbol.for("react.suspense"),_=Symbol.for("react.suspense_list"),g=Symbol.for("react.memo"),w=Symbol.for("react.lazy"),R=Symbol.for("react.view_transition"),$=Symbol.for("react.client.reference");function u(o){if(typeof o=="object"&&o!==null){var T=o.$$typeof;switch(T){case e:switch(o=o.type,o){case r:case i:case a:case m:case _:case R:return o;default:switch(o=o&&o.$$typeof,o){case d:case l:case w:case g:return o;case n:return o;default:return T}}case t:return T}}}return c.ContextConsumer=n,c.ContextProvider=d,c.Element=e,c.ForwardRef=l,c.Fragment=r,c.Lazy=w,c.Memo=g,c.Portal=t,c.Profiler=i,c.StrictMode=a,c.Suspense=m,c.SuspenseList=_,c.isContextConsumer=function(o){return u(o)===n},c.isContextProvider=function(o){return u(o)===d},c.isElement=function(o){return typeof o=="object"&&o!==null&&o.$$typeof===e},c.isForwardRef=function(o){return u(o)===l},c.isFragment=function(o){return u(o)===r},c.isLazy=function(o){return u(o)===w},c.isMemo=function(o){return u(o)===g},c.isPortal=function(o){return u(o)===t},c.isProfiler=function(o){return u(o)===i},c.isStrictMode=function(o){return u(o)===a},c.isSuspense=function(o){return u(o)===m},c.isSuspenseList=function(o){return u(o)===_},c.isValidElementType=function(o){return typeof o=="string"||typeof o=="function"||o===r||o===i||o===a||o===m||o===_||typeof o=="object"&&o!==null&&(o.$$typeof===w||o.$$typeof===g||o.$$typeof===d||o.$$typeof===n||o.$$typeof===l||o.$$typeof===$||o.getModuleId!==void 0)},c.typeOf=u,c}var Se;function Yt(){return Se||(Se=1,de.exports=Ht()),de.exports}var Te=Yt();const zt=/^\s*function(?:\s|\s*\/\*.*\*\/\s*)+([^(\s/]*)\s*/;function lt(e){const t=`${e}`.match(zt);return t&&t[1]||""}function ct(e,t=""){return e.displayName||e.name||lt(e)||t}function Ee(e,t,r){const a=ct(t);return e.displayName||(a!==""?`${r}(${a})`:r)}function Gt(e){if(e!=null){if(typeof e=="string")return e;if(typeof e=="function")return ct(e,"Component");if(typeof e=="object")switch(e.$$typeof){case Te.ForwardRef:return Ee(e,e.render,"ForwardRef");case Te.Memo:return Ee(e,e.type,"memo");default:return}}}const Qt=Object.freeze(Object.defineProperty({__proto__:null,default:Gt,getFunctionName:lt},Symbol.toStringTag,{value:"Module"}));function J(e,t,r=void 0){const a={};return Object.keys(e).forEach(i=>{a[i]=e[i].reduce((n,d)=>{if(d){const l=t(d);l!==""&&n.push(l),r&&r[d]&&n.push(r[d])}return n},[]).join(" ")}),a}var A={},pe={exports:{}},Re;function Jt(){return Re||(Re=1,(function(e){function t(){return e.exports=t=Object.assign?Object.assign.bind():function(r){for(var a=1;a<arguments.length;a++){var i=arguments[a];for(var n in i)({}).hasOwnProperty.call(i,n)&&(r[n]=i[n])}return r},e.exports.__esModule=!0,e.exports.default=e.exports,t.apply(null,arguments)}e.exports=t,e.exports.__esModule=!0,e.exports.default=e.exports})(pe)),pe.exports}var fe={exports:{}},Pe;function Xt(){return Pe||(Pe=1,(function(e){function t(r,a){if(r==null)return{};var i={};for(var n in r)if({}.hasOwnProperty.call(r,n)){if(a.indexOf(n)!==-1)continue;i[n]=r[n]}return i}e.exports=t,e.exports.__esModule=!0,e.exports.default=e.exports})(fe)),fe.exports}const Zt=V(Rt),Kt=V(Nt),er=V(Dt),tr=V(Qt),rr=V(At),or=V(kt);var $e;function ar(){if($e)return A;$e=1;var e=Pt();Object.defineProperty(A,"__esModule",{value:!0}),A.default=pt,A.shouldForwardProp=u,A.systemDefaultTheme=void 0;var t=e(Jt()),r=e(Xt()),a=w(Zt),i=Kt;e(er),e(tr);var n=e(rr),d=e(or);const l=["ownerState"],m=["variants"],_=["name","slot","skipVariantsResolver","skipSx","overridesResolver"];function g(s){if(typeof WeakMap!="function")return null;var y=new WeakMap,p=new WeakMap;return(g=function(b){return b?p:y})(s)}function w(s,y){if(s&&s.__esModule)return s;if(s===null||typeof s!="object"&&typeof s!="function")return{default:s};var p=g(y);if(p&&p.has(s))return p.get(s);var b={__proto__:null},F=Object.defineProperty&&Object.getOwnPropertyDescriptor;for(var v in s)if(v!=="default"&&Object.prototype.hasOwnProperty.call(s,v)){var x=F?Object.getOwnPropertyDescriptor(s,v):null;x&&(x.get||x.set)?Object.defineProperty(b,v,x):b[v]=s[v]}return b.default=s,p&&p.set(s,b),b}function R(s){return Object.keys(s).length===0}function $(s){return typeof s=="string"&&s.charCodeAt(0)>96}function u(s){return s!=="ownerState"&&s!=="theme"&&s!=="sx"&&s!=="as"}function o(s,y){return y&&s&&typeof s=="object"&&s.styles&&!s.styles.startsWith("@layer")&&(s.styles=`@layer ${y}{${String(s.styles)}}`),s}const T=A.systemDefaultTheme=(0,n.default)(),M=s=>s&&s.charAt(0).toLowerCase()+s.slice(1);function X({defaultTheme:s,theme:y,themeId:p}){return R(y)?s:y[p]||y}function dt(s){return s?(y,p)=>p[s]:null}function Z(s,y,p){let{ownerState:b}=y,F=(0,r.default)(y,l);const v=typeof s=="function"?s((0,t.default)({ownerState:b},F)):s;if(Array.isArray(v))return v.flatMap(x=>Z(x,(0,t.default)({ownerState:b},F),p));if(v&&typeof v=="object"&&Array.isArray(v.variants)){const{variants:x=[]}=v;let S=(0,r.default)(v,m);return x.forEach(C=>{let k=!0;if(typeof C.props=="function"?k=C.props((0,t.default)({ownerState:b},F,b)):Object.keys(C.props).forEach(O=>{(b==null?void 0:b[O])!==C.props[O]&&F[O]!==C.props[O]&&(k=!1)}),k){Array.isArray(S)||(S=[S]);const O=typeof C.style=="function"?C.style((0,t.default)({ownerState:b},F,b)):C.style;S.push(p?o((0,a.internal_serializeStyles)(O),p):O)}}),S}return p?o((0,a.internal_serializeStyles)(v),p):v}function pt(s={}){const{themeId:y,defaultTheme:p=T,rootShouldForwardProp:b=u,slotShouldForwardProp:F=u}=s,v=x=>(0,d.default)((0,t.default)({},x,{theme:X((0,t.default)({},x,{defaultTheme:p,themeId:y}))}));return v.__mui_systemSx=!0,(x,ce={})=>{(0,a.internal_processStyles)(x,E=>E.filter(I=>!(I!=null&&I.__mui_systemSx)));const{name:S,slot:C,skipVariantsResolver:k,skipSx:O,overridesResolver:ye=dt(M(C))}=ce,ft=(0,r.default)(ce,_),mt=S&&S.startsWith("Mui")||C?"components":"custom",ht=k!==void 0?k:C&&C!=="Root"&&C!=="root"||!1,yt=O||!1;let vt,K=u;C==="Root"||C==="root"?K=b:C?K=F:$(x)&&(K=void 0);const ue=(0,a.default)(x,(0,t.default)({shouldForwardProp:K,label:vt},ft)),ve=E=>typeof E=="function"&&E.__emotion_real!==E||(0,i.isPlainObject)(E)?I=>{const B=X({theme:I.theme,defaultTheme:p,themeId:y});return Z(E,(0,t.default)({},I,{theme:B}),B.modularCssLayers?mt:void 0)}:E,ge=(E,...I)=>{let B=ve(E);const G=I?I.map(ve):[];S&&ye&&G.push(j=>{const P=X((0,t.default)({},j,{defaultTheme:p,themeId:y}));if(!P.components||!P.components[S]||!P.components[S].styleOverrides)return null;const U=P.components[S].styleOverrides,ee={};return Object.entries(U).forEach(([gt,bt])=>{ee[gt]=Z(bt,(0,t.default)({},j,{theme:P}),P.modularCssLayers?"theme":void 0)}),ye(j,ee)}),S&&!ht&&G.push(j=>{var P;const U=X((0,t.default)({},j,{defaultTheme:p,themeId:y})),ee=U==null||(P=U.components)==null||(P=P[S])==null?void 0:P.variants;return Z({variants:ee},(0,t.default)({},j,{theme:U}),U.modularCssLayers?"theme":void 0)}),yt||G.push(v);const be=G.length-I.length;if(Array.isArray(E)&&be>0){const j=new Array(be).fill("");B=[...E,...j],B.raw=[...E.raw,...j]}const xe=ue(B,...G);return x.muiName&&(xe.muiName=x.muiName),xe};return ue.withConfig&&(ge.withConfig=ue.withConfig),ge}}return A}var nr=ar();const sr=Ft(nr);function ir(e){return e!=="ownerState"&&e!=="theme"&&e!=="sx"&&e!=="as"}const lr=e=>ir(e)&&e!=="classes",D=sr({themeId:st,defaultTheme:$t,rootShouldForwardProp:lr}),Me=e=>{let t;return e<1?t=5.11916*e**2:t=4.5*Math.log(e+1)+2,(t/100).toFixed(2)};function cr(e){return Y("MuiPaper",e)}z("MuiPaper",["root","rounded","outlined","elevation","elevation0","elevation1","elevation2","elevation3","elevation4","elevation5","elevation6","elevation7","elevation8","elevation9","elevation10","elevation11","elevation12","elevation13","elevation14","elevation15","elevation16","elevation17","elevation18","elevation19","elevation20","elevation21","elevation22","elevation23","elevation24"]);const ur=["className","component","elevation","square","variant"],dr=e=>{const{square:t,elevation:r,variant:a,classes:i}=e,n={root:["root",a,!t&&"rounded",a==="elevation"&&`elevation${r}`]};return J(n,cr,i)},pr=D("div",{name:"MuiPaper",slot:"Root",overridesResolver:(e,t)=>{const{ownerState:r}=e;return[t.root,t[r.variant],!r.square&&t.rounded,r.variant==="elevation"&&t[`elevation${r.elevation}`]]}})(({theme:e,ownerState:t})=>{var r;return h({backgroundColor:(e.vars||e).palette.background.paper,color:(e.vars||e).palette.text.primary,transition:e.transitions.create("box-shadow")},!t.square&&{borderRadius:e.shape.borderRadius},t.variant==="outlined"&&{border:`1px solid ${(e.vars||e).palette.divider}`},t.variant==="elevation"&&h({boxShadow:(e.vars||e).shadows[t.elevation]},!e.vars&&e.palette.mode==="dark"&&{backgroundImage:`linear-gradient(${Ce.alpha("#fff",Me(t.elevation))}, ${Ce.alpha("#fff",Me(t.elevation))})`},e.vars&&{backgroundImage:(r=e.vars.overlays)==null?void 0:r[t.elevation]}))}),fr=L.forwardRef(function(t,r){const a=Q({props:t,name:"MuiPaper"}),{className:i,component:n="div",elevation:d=1,square:l=!1,variant:m="elevation"}=a,_=q(a,ur),g=h({},a,{component:n,elevation:d,square:l,variant:m}),w=dr(g);return f.jsx(pr,h({as:n,ownerState:g,className:H(w.root,i),ref:r},_))});function mr(e){return Y("MuiTypography",e)}z("MuiTypography",["root","h1","h2","h3","h4","h5","h6","subtitle1","subtitle2","body1","body2","inherit","button","caption","overline","alignLeft","alignRight","alignCenter","alignJustify","noWrap","gutterBottom","paragraph"]);const hr=["align","className","component","gutterBottom","noWrap","paragraph","variant","variantMapping"],yr=e=>{const{align:t,gutterBottom:r,noWrap:a,paragraph:i,variant:n,classes:d}=e,l={root:["root",n,e.align!=="inherit"&&`align${me(t)}`,r&&"gutterBottom",a&&"noWrap",i&&"paragraph"]};return J(l,mr,d)},vr=D("span",{name:"MuiTypography",slot:"Root",overridesResolver:(e,t)=>{const{ownerState:r}=e;return[t.root,r.variant&&t[r.variant],r.align!=="inherit"&&t[`align${me(r.align)}`],r.noWrap&&t.noWrap,r.gutterBottom&&t.gutterBottom,r.paragraph&&t.paragraph]}})(({theme:e,ownerState:t})=>h({margin:0},t.variant==="inherit"&&{font:"inherit"},t.variant!=="inherit"&&e.typography[t.variant],t.align!=="inherit"&&{textAlign:t.align},t.noWrap&&{overflow:"hidden",textOverflow:"ellipsis",whiteSpace:"nowrap"},t.gutterBottom&&{marginBottom:"0.35em"},t.paragraph&&{marginBottom:16})),Fe={h1:"h1",h2:"h2",h3:"h3",h4:"h4",h5:"h5",h6:"h6",subtitle1:"h6",subtitle2:"h6",body1:"p",body2:"p",inherit:"p"},gr={primary:"primary.main",textPrimary:"text.primary",secondary:"secondary.main",textSecondary:"text.secondary",error:"error.main"},br=e=>gr[e]||e,N=L.forwardRef(function(t,r){const a=Q({props:t,name:"MuiTypography"}),i=br(a.color),n=he(h({},a,{color:i})),{align:d="inherit",className:l,component:m,gutterBottom:_=!1,noWrap:g=!1,paragraph:w=!1,variant:R="body1",variantMapping:$=Fe}=n,u=q(n,hr),o=h({},n,{align:d,color:i,className:l,component:m,gutterBottom:_,noWrap:g,paragraph:w,variant:R,variantMapping:$}),T=m||(w?"p":$[R]||Fe[R])||"span",M=yr(o);return f.jsx(vr,h({as:T,ref:r,ownerState:o,className:H(M.root,l)},u))}),xr=z("MuiBox",["root"]),Cr=Mt(),wr=Lt({themeId:st,defaultTheme:Cr,defaultClassName:xr.root,generateClassName:it.generate});function _r(e){return Y("MuiCard",e)}z("MuiCard",["root"]);const Sr=["className","raised"],Tr=e=>{const{classes:t}=e;return J({root:["root"]},_r,t)},Er=D(fr,{name:"MuiCard",slot:"Root",overridesResolver:(e,t)=>t.root})(()=>({overflow:"hidden"})),Rr=L.forwardRef(function(t,r){const a=Q({props:t,name:"MuiCard"}),{className:i,raised:n=!1}=a,d=q(a,Sr),l=h({},a,{raised:n}),m=Tr(l);return f.jsx(Er,h({className:H(m.root,i),elevation:n?8:void 0,ref:r,ownerState:l},d))});function Pr(e){return Y("MuiCardContent",e)}z("MuiCardContent",["root"]);const $r=["className","component"],Mr=e=>{const{classes:t}=e;return J({root:["root"]},Pr,t)},Fr=D("div",{name:"MuiCardContent",slot:"Root",overridesResolver:(e,t)=>t.root})(()=>({padding:16,"&:last-child":{paddingBottom:24}})),Or=L.forwardRef(function(t,r){const a=Q({props:t,name:"MuiCardContent"}),{className:i,component:n="div"}=a,d=q(a,$r),l=h({},a,{component:n}),m=Mr(l);return f.jsx(Fr,h({as:n,className:H(m.root,i),ownerState:l,ref:r},d))});function Ir(e){return Y("MuiCardHeader",e)}const Oe=z("MuiCardHeader",["root","avatar","action","content","title","subheader"]),jr=["action","avatar","className","component","disableTypography","subheader","subheaderTypographyProps","title","titleTypographyProps"],Nr=e=>{const{classes:t}=e;return J({root:["root"],avatar:["avatar"],action:["action"],content:["content"],title:["title"],subheader:["subheader"]},Ir,t)},Dr=D("div",{name:"MuiCardHeader",slot:"Root",overridesResolver:(e,t)=>h({[`& .${Oe.title}`]:t.title,[`& .${Oe.subheader}`]:t.subheader},t.root)})({display:"flex",alignItems:"center",padding:16}),Ar=D("div",{name:"MuiCardHeader",slot:"Avatar",overridesResolver:(e,t)=>t.avatar})({display:"flex",flex:"0 0 auto",marginRight:16}),qr=D("div",{name:"MuiCardHeader",slot:"Action",overridesResolver:(e,t)=>t.action})({flex:"0 0 auto",alignSelf:"flex-start",marginTop:-4,marginRight:-8,marginBottom:-4}),Wr=D("div",{name:"MuiCardHeader",slot:"Content",overridesResolver:(e,t)=>t.content})({flex:"1 1 auto"}),kr=L.forwardRef(function(t,r){const a=Q({props:t,name:"MuiCardHeader"}),{action:i,avatar:n,className:d,component:l="div",disableTypography:m=!1,subheader:_,subheaderTypographyProps:g,title:w,titleTypographyProps:R}=a,$=q(a,jr),u=h({},a,{component:l,disableTypography:m}),o=Nr(u);let T=w;T!=null&&T.type!==N&&!m&&(T=f.jsx(N,h({variant:n?"body2":"h5",className:o.title,component:"span",display:"block"},R,{children:T})));let M=_;return M!=null&&M.type!==N&&!m&&(M=f.jsx(N,h({variant:n?"body2":"body1",className:o.subheader,color:"text.secondary",component:"span",display:"block"},g,{children:M}))),f.jsxs(Dr,h({className:H(o.root,d),as:l,ref:r,ownerState:u},$,{children:[n&&f.jsx(Ar,{className:o.avatar,ownerState:u,children:n}),f.jsxs(Wr,{className:o.content,ownerState:u,children:[T,M]}),i&&f.jsx(qr,{className:o.action,ownerState:u,children:i})]}))}),ut=({companyId:e,userType:t="intermediate",showEducationalContent:r=!0,showCollaborativeFeatures:a=!0})=>f.jsx(wr,{sx:{p:3},children:f.jsxs(Rr,{children:[f.jsx(kr,{title:"Financial Dashboard"}),f.jsxs(Or,{children:[f.jsxs(N,{variant:"h6",gutterBottom:!0,children:["Company ID: ",e]}),f.jsxs(N,{variant:"body1",gutterBottom:!0,children:["User Type: ",t]}),f.jsxs(N,{variant:"body1",gutterBottom:!0,children:["Educational Content: ",r?"Enabled":"Disabled"]}),f.jsxs(N,{variant:"body1",children:["Collaborative Features: ",a?"Enabled":"Disabled"]})]})]})});ut.__docgenInfo={description:"",methods:[],displayName:"FinancialDashboardSimple",props:{companyId:{required:!0,tsType:{name:"string"},description:""},userType:{required:!1,tsType:{name:"union",raw:"'beginner' | 'intermediate' | 'advanced' | 'expert'",elements:[{name:"literal",value:"'beginner'"},{name:"literal",value:"'intermediate'"},{name:"literal",value:"'advanced'"},{name:"literal",value:"'expert'"}]},description:"",defaultValue:{value:"'intermediate'",computed:!1}},showEducationalContent:{required:!1,tsType:{name:"boolean"},description:"",defaultValue:{value:"true",computed:!1}},showCollaborativeFeatures:{required:!1,tsType:{name:"boolean"},description:"",defaultValue:{value:"true",computed:!1}}}};console.log("🔧 FinancialDashboard.stories.tsx: File loaded successfully");const W="mock-company-id",Br=()=>new It({defaultOptions:{queries:{retry:!1,refetchOnWindowFocus:!1,suspense:!0}}});console.log("🔧 FinancialDashboard.stories.tsx: Creating meta object");const Gr={title:"Financial/FinancialDashboard",component:ut,parameters:{layout:"fullscreen",docs:{description:{component:"A comprehensive financial dashboard that provides an overview of company financial data with key metrics, recent filings, and interactive navigation."}}},decorators:[e=>{const t=Br();return f.jsx(jt,{client:t,children:f.jsx(Ot,{children:f.jsx(e,{})})})}],argTypes:{companyId:{control:"text",description:"The company ID to display financial data for"},userType:{control:"select",options:["beginner","intermediate","advanced","expert"],description:"The user experience level to customize the interface"},showEducationalContent:{control:"boolean",description:"Whether to show educational content and tooltips"},showCollaborativeFeatures:{control:"boolean",description:"Whether to show collaborative features like sharing and comments"}}};console.log("🔧 FinancialDashboard.stories.tsx: Exporting meta and creating Story type");console.log("🔧 FinancialDashboard.stories.tsx: Creating Default story");const te={args:{companyId:W,userType:"intermediate",showEducationalContent:!0,showCollaborativeFeatures:!0},parameters:{msw:{handlers:"success"}}},re={args:{companyId:W,userType:"beginner",showEducationalContent:!0,showCollaborativeFeatures:!1},parameters:{msw:{handlers:"success"},docs:{description:{story:"Dashboard optimized for beginner users with educational content and simplified interface."}}}},oe={args:{companyId:W,userType:"advanced",showEducationalContent:!1,showCollaborativeFeatures:!0},parameters:{docs:{description:{story:"Dashboard for advanced users with collaborative features and detailed analytics."}}}},ae={args:{companyId:W,userType:"expert",showEducationalContent:!1,showCollaborativeFeatures:!0},parameters:{docs:{description:{story:"Dashboard for expert users with full feature set and professional tools."}}}},ne={args:{companyId:W,userType:"intermediate",showEducationalContent:!0,showCollaborativeFeatures:!0},parameters:{msw:{handlers:"success"},viewport:{defaultViewport:"mobile"},docs:{description:{story:"Dashboard optimized for mobile devices with responsive design."}}}},se={args:{companyId:W,userType:"intermediate",showEducationalContent:!0,showCollaborativeFeatures:!0},parameters:{msw:{handlers:"success"},viewport:{defaultViewport:"tablet"},docs:{description:{story:"Dashboard optimized for tablet devices with touch-friendly interface."}}}},ie={args:{companyId:W,userType:"intermediate",showEducationalContent:!0,showCollaborativeFeatures:!0},parameters:{msw:{handlers:"loading"},docs:{description:{story:"Dashboard in loading state while fetching financial data."}}}},le={args:{companyId:"invalid-company-id",userType:"intermediate",showEducationalContent:!0,showCollaborativeFeatures:!0},parameters:{msw:{handlers:"error"},docs:{description:{story:"Dashboard showing error state when company data cannot be loaded."}}}};var Ie,je,Ne;te.parameters={...te.parameters,docs:{...(Ie=te.parameters)==null?void 0:Ie.docs,source:{originalSource:`{
  args: {
    companyId: mockCompanyId,
    userType: 'intermediate',
    showEducationalContent: true,
    showCollaborativeFeatures: true
  },
  parameters: {
    msw: {
      handlers: 'success' // Use existing MSW success handlers
    }
  }
}`,...(Ne=(je=te.parameters)==null?void 0:je.docs)==null?void 0:Ne.source}}};var De,Ae,qe;re.parameters={...re.parameters,docs:{...(De=re.parameters)==null?void 0:De.docs,source:{originalSource:`{
  args: {
    companyId: mockCompanyId,
    userType: 'beginner',
    showEducationalContent: true,
    showCollaborativeFeatures: false
  },
  parameters: {
    msw: {
      handlers: 'success' // Use existing MSW success handlers
    },
    docs: {
      description: {
        story: 'Dashboard optimized for beginner users with educational content and simplified interface.'
      }
    }
  }
}`,...(qe=(Ae=re.parameters)==null?void 0:Ae.docs)==null?void 0:qe.source}}};var We,ke,Be;oe.parameters={...oe.parameters,docs:{...(We=oe.parameters)==null?void 0:We.docs,source:{originalSource:`{
  args: {
    companyId: mockCompanyId,
    userType: 'advanced',
    showEducationalContent: false,
    showCollaborativeFeatures: true
  },
  parameters: {
    docs: {
      description: {
        story: 'Dashboard for advanced users with collaborative features and detailed analytics.'
      }
    }
  }
}`,...(Be=(ke=oe.parameters)==null?void 0:ke.docs)==null?void 0:Be.source}}};var Ue,Le,Ve;ae.parameters={...ae.parameters,docs:{...(Ue=ae.parameters)==null?void 0:Ue.docs,source:{originalSource:`{
  args: {
    companyId: mockCompanyId,
    userType: 'expert',
    showEducationalContent: false,
    showCollaborativeFeatures: true
  },
  parameters: {
    docs: {
      description: {
        story: 'Dashboard for expert users with full feature set and professional tools.'
      }
    }
  }
}`,...(Ve=(Le=ae.parameters)==null?void 0:Le.docs)==null?void 0:Ve.source}}};var He,Ye,ze;ne.parameters={...ne.parameters,docs:{...(He=ne.parameters)==null?void 0:He.docs,source:{originalSource:`{
  args: {
    companyId: mockCompanyId,
    userType: 'intermediate',
    showEducationalContent: true,
    showCollaborativeFeatures: true
  },
  parameters: {
    msw: {
      handlers: 'success' // Use existing MSW success handlers
    },
    viewport: {
      defaultViewport: 'mobile'
    },
    docs: {
      description: {
        story: 'Dashboard optimized for mobile devices with responsive design.'
      }
    }
  }
}`,...(ze=(Ye=ne.parameters)==null?void 0:Ye.docs)==null?void 0:ze.source}}};var Ge,Qe,Je;se.parameters={...se.parameters,docs:{...(Ge=se.parameters)==null?void 0:Ge.docs,source:{originalSource:`{
  args: {
    companyId: mockCompanyId,
    userType: 'intermediate',
    showEducationalContent: true,
    showCollaborativeFeatures: true
  },
  parameters: {
    msw: {
      handlers: 'success' // Use existing MSW success handlers
    },
    viewport: {
      defaultViewport: 'tablet'
    },
    docs: {
      description: {
        story: 'Dashboard optimized for tablet devices with touch-friendly interface.'
      }
    }
  }
}`,...(Je=(Qe=se.parameters)==null?void 0:Qe.docs)==null?void 0:Je.source}}};var Xe,Ze,Ke;ie.parameters={...ie.parameters,docs:{...(Xe=ie.parameters)==null?void 0:Xe.docs,source:{originalSource:`{
  args: {
    companyId: mockCompanyId,
    userType: 'intermediate',
    showEducationalContent: true,
    showCollaborativeFeatures: true
  },
  parameters: {
    msw: {
      handlers: 'loading' // Use existing MSW loading handlers
    },
    docs: {
      description: {
        story: 'Dashboard in loading state while fetching financial data.'
      }
    }
  }
}`,...(Ke=(Ze=ie.parameters)==null?void 0:Ze.docs)==null?void 0:Ke.source}}};var et,tt,rt;le.parameters={...le.parameters,docs:{...(et=le.parameters)==null?void 0:et.docs,source:{originalSource:`{
  args: {
    companyId: 'invalid-company-id',
    userType: 'intermediate',
    showEducationalContent: true,
    showCollaborativeFeatures: true
  },
  parameters: {
    msw: {
      handlers: 'error' // Use existing MSW error handlers
    },
    docs: {
      description: {
        story: 'Dashboard showing error state when company data cannot be loaded.'
      }
    }
  }
}`,...(rt=(tt=le.parameters)==null?void 0:tt.docs)==null?void 0:rt.source}}};const Qr=["Default","BeginnerUser","AdvancedUser","ExpertUser","MobileView","TabletView","LoadingState","ErrorState"];export{oe as AdvancedUser,re as BeginnerUser,te as Default,le as ErrorState,ae as ExpertUser,ie as LoadingState,ne as MobileView,se as TabletView,Qr as __namedExportsOrder,Gr as default};
