export interface FinancialStatement {
  id: string;
  companyId: string;
  filingType: string;
  formType: string;
  accessionNumber: string;
  filingDate: string;
  periodEndDate: string;
  fiscalYear: number;
  fiscalQuarter?: number;
  documentType: string;
  documentUrl: string;
  xbrlProcessingStatus: string;
  xbrlProcessingError?: string;
  xbrlProcessingStartedAt?: string;
  xbrlProcessingCompletedAt?: string;
  isAmended: boolean;
  amendmentType?: string;
  originalFilingDate?: string;
  isRestated: boolean;
  restatementReason?: string;
  lineItems?: FinancialLineItem[];
  createdAt: string;
  updatedAt: string;
}

export interface FinancialLineItem {
  id: string;
  statementId: string;
  taxonomyConcept?: string;
  standardLabel?: string;
  value?: number;
  unit: string;
  contextRef: string;
  statementType: string;
  statementSection: string;
  isCalculated: boolean;
  calculationWeight?: number;
  parentConcept?: string;
  createdAt: string;
  updatedAt: string;
}

export interface FinancialRatio {
  id: string;
  statementId: string;
  ratioName: string;
  ratioDisplayName: string;
  value: number;
  category: string;
  formula: string;
  interpretation: string;
  benchmarkPercentile?: number;
  periodEndDate: string;
  fiscalYear: number;
  fiscalQuarter?: number;
  calculatedAt: string;
  dataQualityScore: number;
}

export interface Company {
  id: string;
  cik: string;
  name: string;
  ticker?: string;
  sic?: string;
  sicDescription?: string;
  gics?: string;
  gicsDescription?: string;
  businessAddress?: any;
  mailingAddress?: any;
  phone?: string;
  website?: string;
  stateOfIncorporation?: string;
  stateOfIncorporationDescription?: string;
  fiscalYearEnd?: string;
  businessStatus?: string;
  createdAt: string;
  updatedAt: string;
}

export interface FinancialAnnotation {
  id: string;
  statementId: string;
  lineItemId?: string;
  authorId: string;
  content: string;
  annotationType: string;
  tags: string[];
  highlights?: any;
  mentions: string[];
  parentAnnotationId?: string;
  status: string;
  isPrivate: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface AnnotationReply {
  id: string;
  annotationId: string;
  authorId: string;
  content: string;
  mentions: string[];
  createdAt: string;
  updatedAt: string;
}

export interface AnnotationAssignment {
  id: string;
  statementId: string;
  lineItemId?: string;
  assigneeId: string;
  assignerId: string;
  assignmentType: string;
  dueDate?: string;
  status: string;
  notes?: string;
  createdAt: string;
  updatedAt: string;
}

export interface AnnotationTemplate {
  id: string;
  name: string;
  description?: string;
  templateContent: string;
  annotationType: string;
  tags: string[];
  isPublic: boolean;
  createdBy: string;
  usageCount: number;
  createdAt: string;
  updatedAt: string;
}
