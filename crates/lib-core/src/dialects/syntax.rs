use std::iter::FromIterator;

#[derive(
    Debug,
    PartialEq,
    Eq,
    Clone,
    Copy,
    strum_macros::IntoStaticStr,
    strum_macros::EnumString,
    Hash,
    PartialOrd,
    Ord,
    Default,
)]
#[strum(serialize_all = "snake_case")]
pub enum SyntaxKind {
    Unparsable,
    File,
    ColumnReference,
    ObjectReference,
    Expression,
    WildcardIdentifier,
    Function,
    HavingClause,
    PathSegment,
    LimitClause,
    CubeRollupClause,
    GroupingSetsClause,
    GroupingExpressionList,
    SetClause,
    FetchClause,
    FunctionDefinition,
    AlterSequenceOptionsSegment,
    RoleReference,
    TablespaceReference,
    ExtensionReference,
    TagReference,
    ColumnDefinition,
    ColumnConstraintSegment,
    CommentClause,
    TableEndClause,
    MergeMatch,
    MergeWhenNotMatchedClause,
    MergeInsertClause,
    MergeUpdateClause,
    MergeDeleteClause,
    SetClauseList,
    TableReference,
    GroupbyClause,
    FrameClause,
    WithCompoundStatement,
    CommonTableExpression,
    CTEColumnList,
    TriggerReference,
    TableConstraint,
    JoinOnCondition,
    DatabaseReference,
    CollationReference,
    OverClause,
    NamedWindow,
    WindowSpecification,
    PartitionbyClause,
    JoinClause,
    DropTriggerStatement,
    SampleExpression,
    TableExpression,
    CreateTriggerStatement,
    DropModelStatement,
    DescribeStatement,
    UseStatement,
    ExplainStatement,
    CreateSequenceStatement,
    CreateSequenceOptionsSegment,
    AlterSequenceStatement,
    DropSequenceStatement,
    DropCastStatement,
    CreateFunctionStatement,
    DropFunctionStatement,
    CreateModelStatement,
    CreateViewStatement,
    DeleteStatement,
    UpdateStatement,
    CreateCastStatement,
    CreateRoleStatement,
    DropRoleStatement,
    AlterTableStatement,
    CreateSchemaStatement,
    SetSchemaStatement,
    DropSchemaStatement,
    DropTypeStatement,
    CreateDatabaseStatement,
    DropDatabaseStatement,
    FunctionParameterList,
    CreateIndexStatement,
    DropIndexStatement,
    CreateTableStatement,
    AccessStatement,
    InsertStatement,
    TransactionStatement,
    DropTableStatement,
    DropViewStatement,
    CreateUserStatement,
    DropUserStatement,
    ArrayExpression,
    LocalAlias,
    MergeStatement,
    IndexColumnDefinition,
    AggregateOrderByClause,
    FunctionName,
    CaseExpression,
    WhenClause,
    ElseClause,
    WhereClause,
    SetOperator,
    ValuesClause,
    EmptyStructLiteral,
    ObjectLiteral,
    ObjectLiteralElement,
    TimeZoneGrammar,
    BracketedArguments,
    DataType,
    AliasExpression,
    ArrayAccessor,
    ArrayLiteral,
    TypedArrayLiteral,
    StructType,
    StructLiteral,
    TypedStructLiteral,
    IntervalExpression,
    ArrayType,
    SizedArrayType,
    SelectStatement,
    OverlapsClause,
    SelectClause,
    Statement,
    WithNoSchemaBindingClause,
    WithDataClause,
    SetExpression,
    FromClause,
    EmptyStructLiteralBrackets,
    WildcardExpression,
    OrderbyClause,
    TruncateStatement,
    FromExpression,
    FromExpressionElement,
    SelectClauseModifier,
    NamedWindowExpression,
    SelectClauseElement,
    QualifyClause,
    MultiStatementSegment,
    AssertStatement,
    ForInStatements,
    ForInStatement,
    RepeatStatements,
    RepeatStatement,
    IfStatements,
    IfStatement,
    LoopStatements,
    LoopStatement,
    WhileStatements,
    WhileStatement,
    DatePartWeek,
    SelectExceptClause,
    SelectReplaceClause,
    StructTypeSchema,
    Tuple,
    NamedArgument,
    DeclareSegment,
    SetSegment,
    PartitionBySegment,
    ClusterBySegment,
    OptionsSegment,
    CreateExternalTableStatement,
    AlterViewStatement,
    CreateMaterializedViewStatement,
    AlterMaterializedViewSetOptionsStatement,
    DropMaterializedViewStatement,
    ParameterizedExpression,
    PivotForClause,
    FromPivotExpression,
    UnpivotClause,
    FromUnpivotExpression,
    NotMatchedByTargetClause,
    MergeWhenMatchedClause,
    ProcedureName,
    ExportStatement,
    ProcedureParameterList,
    ProcedureStatements,
    CallStatement,
    ReturnStatement,
    BreakStatement,
    LeaveStatement,
    ContinueStatement,
    RaiseStatement,
    PsqlVariable,
    ComparisonOperator,
    DatetimeTypeIdentifier,
    DatetimeLiteral,
    IndexAccessMethod,
    OperatorClassReference,
    DefinitionParameter,
    DefinitionParameters,
    RelationOption,
    RelationOptions,
    AlterFunctionActionSegment,
    AlterProcedureActionSegment,
    AlterProcedureStatement,
    DropProcedureStatement,
    WktGeometryType,
    IntoClause,
    ForClause,
    AlterRoleStatement,
    ExplainOption,
    CreateTableAsStatement,
    AlterPublicationStatement,
    CreatePublicationStatement,
    PublicationObjects,
    PublicationTable,
    PublicationReference,
    DropExtensionStatement,
    CreateExtensionStatement,
    VersionIdentifier,
    AlterTableActionSegment,
    DropPublicationStatement,
    AlterMaterializedViewStatement,
    AlterMaterializedViewActionSegment,
    RefreshMaterializedViewStatement,
    WithCheckOption,
    AlterPolicyStatement,
    AlterDatabaseStatement,
    VacuumStatement,
    LikeOptionSegment,
    PartitionBoundSpec,
    IndexParameters,
    ReferentialActionSegment,
    IndexElement,
    ExclusionConstraintElement,
    AlterDefaultPrivilegesStatement,
    AlterDefaultPrivilegesObjectPrivilege,
    AlterDefaultPrivilegesSchemaObject,
    AlterDefaultPrivilegesToFromRoles,
    AlterDefaultPrivilegesGrant,
    DropOwnedStatement,
    ReassignOwnedStatement,
    IndexElementOptions,
    AlterDefaultPrivilegesRevoke,
    AlterIndexStatement,
    ReindexStatementSegment,
    AnalyzeStatement,
    AlterTrigger,
    OperationClassReference,
    ConflictAction,
    ConflictTarget,
    SetStatement,
    CreatePolicyStatement,
    CreateDomainStatement,
    AlterDomainStatement,
    DropDomainStatement,
    DropPolicyStatement,
    LoadStatement,
    ResetStatement,
    ListenStatement,
    NotifyStatement,
    UnlistenStatement,
    ClusterStatement,
    LanguageClause,
    DoStatement,
    CreateUserMappingStatement,
    ImportForeignSchemaStatement,
    CreateServerStatement,
    CreateCollationStatement,
    AlterTypeStatement,
    CreateTypeStatement,
    LockTableStatement,
    CopyStatement,
    DiscardStatement,
    AlterSchemaStatement,
    ServerReference,
    ArrayJoinClause,
    TableEngineFunction,
    OnClusterClause,
    Engine,
    EngineFunction,
    DatabaseEngine,
    ColumnTtlSegment,
    TableTtlSegment,
    DropDictionaryStatement,
    DropQuotaStatement,
    DropSettingProfileStatement,
    SystemMergesSegment,
    SystemTtlMergesSegment,
    SystemMovesSegment,
    SystemReplicaSegment,
    SystemFilesystemSegment,
    SystemReplicatedSegment,
    SystemReplicationSegment,
    SystemFetchesSegment,
    SystemDistributedSegment,
    SystemModelSegment,
    SystemFileSegment,
    SystemUnfreezeSegment,
    SystemStatement,
    ConnectbyClause,
    CallSegment,
    WithingroupClause,
    PatternExpression,
    MatchRecognizeClause,
    ChangesClause,
    FromAtExpression,
    FromBeforeExpression,
    SnowflakeKeywordExpression,
    SemiStructuredExpression,
    SelectExcludeClause,
    SelectRenameClause,
    AlterTableTableColumnAction,
    AlterTableClusteringAction,
    AlterTableConstraintAction,
    AlterWarehouseStatement,
    AlterShareStatement,
    AlterStorageIntegrationStatement,
    AlterExternalTableStatement,
    CommentEqualsClause,
    TagBracketedEquals,
    TagEquals,
    CreateCloneStatement,
    CreateDatabaseFromShareStatement,
    CreateProcedureStatement,
    ScriptingBlockStatement,
    ScriptingLetStatement,
    AlterFunctionStatement,
    CreateExternalFunctionStatement,
    WarehouseObjectProperties,
    ConstraintPropertiesSegment,
    CopyOptions,
    SchemaObjectProperties,
    CreateTaskStatement,
    SnowflakeTaskExpressionSegment,
    CreateStatement,
    CreateFileFormatSegment,
    AlterFileFormatSegment,
    CsvFileFormatTypeParameters,
    JsonFileFormatTypeParameters,
    AvroFileFormatTypeParameters,
    OrcFileFormatTypeParameters,
    ParquetFileFormatTypeParameters,
    XmlFileFormatTypeParameters,
    AlterPipeSegment,
    FileFormatSegment,
    FormatTypeOptions,
    CopyIntoLocationStatement,
    CopyIntoTableStatement,
    StorageLocation,
    StageParameters,
    S3ExternalStageParameters,
    GcsExternalStageParameters,
    AzureBlobStorageExternalStageParameters,
    CreateStageStatement,
    AlterStageStatement,
    CreateStreamStatement,
    AlterStreamStatement,
    ShowStatement,
    AlterUserStatement,
    AlterSessionStatement,
    AlterSessionSetStatement,
    AlterSessionUnsetClause,
    AlterTaskStatement,
    AlterTaskSpecialSetClause,
    AlterTaskSetClause,
    AlterTaskUnsetClause,
    ExecuteTaskClause,
    UndropStatement,
    CommentStatement,
    DropExternalTableStatement,
    ListStatement,
    GetStatement,
    PutStatement,
    RemoveStatement,
    CastExpression,
    DropObjectStatement,
    UnsetStatement,
    SqlConfOption,
    BinaryOperator,
    PrimitiveType,
    CreateWidgetStatement,
    ReplaceTableStatement,
    RemoveWidgetStatement,
    UseDatabaseStatement,
    InsertOverwriteDirectoryStatement,
    InsertOverwriteDirectoryHiveFmtStatement,
    LoadDataStatement,
    ClusterByClause,
    DistributeByClause,
    HintFunction,
    SelectHint,
    WithCubeRollupClause,
    SortByClause,
    LateralViewClause,
    PivotClause,
    TransformClause,
    AddFileStatement,
    AddJarStatement,
    AnalyzeTableStatement,
    CacheTable,
    ClearCache,
    ListFileStatement,
    ListJarStatement,
    RefreshStatement,
    UncacheTable,
    FileReference,
    PropertyNameIdentifier,
    GeneratedColumnDefinition,
    IntervalLiteral,
    DescribeHistoryStatement,
    DescribeDetailStatement,
    GenerateManifestFileStatement,
    ConvertToDeltaStatement,
    RestoreTableStatement,
    ConstraintStatement,
    ApplyChangesIntoStatement,
    UsingClause,
    DataSourceFormat,
    IcebergTransformation,
    MsckRepairTableStatement,
    RowFormatClause,
    SkewedByClause,
    Bracketed,
    NumericLiteral,
    Keyword,
    EndOfFile,
    Whitespace,
    Newline,
    NakedIdentifier,
    Unlexable,
    StartBracket,
    EndBracket,
    InlineComment,
    Identifier,
    Raw,
    QuotedIdentifier,
    Star,
    Dot,
    Comma,
    Comment,
    EmitsSegment,
    Literal,
    BareFunction,
    NullLiteral,
    BooleanLiteral,
    BlockComment,
    QuotedLiteral,
    DoubleDivide,
    Meta,
    #[default]
    Colon,
    StatementTerminator,
    StartSquareBracket,
    EndSquareBracket,
    StartCurlyBracket,
    Tilde,
    CastingOperator,
    RawComparisonOperator,
    DatePart,
    Pipe,
    SignIndicator,
    LikeOperator,
    Word,
    DoubleQuote,
    SingleQuote,
    Dash,
    Semicolon,
    BackQuote,
    DollarQuote,
    Not,
    Ampersand,
    Question,
    Percent,
    Divide,
    Minus,
    Plus,
    Caret,
    VerticalBar,
    EndCurlyBracket,
    FunctionNameIdentifier,
    Dedent,
    Indent,
    Implicit,
    AtSignLiteral,
    QuestionMark,
    RightArrow,
    UdfBody,
    StartAngleBracket,
    EndAngleBracket,
    Lambda,
    NakedIdentifierAll,
    ProcedureNameIdentifier,
    Parameter,
    DateConstructorLiteral,
    ProcedureOption,
    ExportOption,
    PropertiesNakedIdentifier,
    Symbol,
    DataTypeIdentifier,
    Placeholder,
    ExecuteScriptStatement,
    AssignmentOperator,
    Batch,
    PivotColumnReference,
    IntoTableClause,
    PasswordAuth,
    ExecuteAsClause,
    UnicodeSingleQuote,
    EscapedSingleQuote,
    UnicodeDoubleQuote,
    JsonOperator,
    At,
    BitStringLiteral,
    DollarNumericLiteral,
    WidgetNameIdentifier,
    FileKeyword,
    SemiStructuredElement,
    BytesDoubleQuote,
    BytesSingleQuote,
    FileFormat,
    FileType,
    StartHint,
    EndHint,
    FunctionAssigner,
    UnquotedFilePath,
    Dollar,
    SystemFunctionName,
    IntegerLiteral,
    StageEncryptionOption,
    BucketPath,
    QuotedStar,
    StagePath,
    FileLiteral,
    BytesQuotedLiteral,
    SignedQuotedLiteral,
    ParameterAssigner,
    ColumnSelector,
    DollarLiteral,
    ExcludeBracketClose,
    WalrusOperator,
    WarehouseSize,
    Variable,
    ExcludeBracketOpen,
    SymlinkFormatManifest,
    StartExcludeBracket,
    CompressionType,
    CopyOnErrorOption,
    ColumnIndexIdentifierSegment,
    ScalingPolicy,
    ValidationModeOption,
    EndExcludeBracket,
    IdentifierList,
    TemplateLoop,
    ColonDelimiter,
    SqlcmdOperator,
    Slice,
    TableEndClauseSegment,
    PragmaStatement,
    PragmaReference,
    Slash,
    DataFormatSegment,
    AuthorizationSegment,
    ColumnAttributeSegment,
    ShowModelStatement,
    CreateExternalSchemaStatement,
    CreateLibraryStatement,
    UnloadStatement,
    DeclareStatement,
    FetchStatement,
    CloseStatement,
    CreateDatashareStatement,
    DescDatashareStatement,
    DropDatashareStatement,
    ShowDatasharesStatement,
    GrantDatashareStatement,
    CreateRlsPolicyStatement,
    ManageRlsPolicyStatement,
    DropRlsPolicyStatement,
    AnalyzeCompressionStatement,
    PartitionedBySegment,
    RowFormatDelimitedSegment,
    ObjectUnpivoting,
    ArrayUnnesting,
    AlterGroup,
    CreateGroup,
    ListaggOverflowClauseSegment,
    UnorderedSelectStatementSegment,
    MapType,
    MapTypeSchema,
    PrepareStatement,
    ExecuteStatement,
}

impl SyntaxKind {
    pub fn indent_val(self) -> i8 {
        match self {
            SyntaxKind::Indent | SyntaxKind::Implicit => 1,
            SyntaxKind::Dedent => -1,
            _ => 0,
        }
    }
}

impl SyntaxKind {
    pub fn as_str(&self) -> &'static str {
        self.into()
    }
}

#[derive(Clone, PartialEq, Eq, Default)]
pub struct SyntaxSet([u64; 10]);

impl std::fmt::Debug for SyntaxSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

impl SyntaxSet {
    pub const EMPTY: SyntaxSet = Self([0; 10]);
    const SLICE_BITS: u16 = u64::BITS as u16;

    pub const fn new(kinds: &[SyntaxKind]) -> Self {
        let mut set = SyntaxSet::EMPTY;

        let mut index = 0;
        while index < kinds.len() {
            set = set.union(&Self::single(kinds[index]));
            index += 1;
        }

        set
    }

    pub const fn single(kind: SyntaxKind) -> Self {
        let kind = kind as u16;

        let index = (kind / Self::SLICE_BITS) as usize;

        debug_assert!(
            index < Self::EMPTY.0.len(),
            "Index out of bounds. Increase the size of the bitset array."
        );

        let shift = kind % Self::SLICE_BITS;
        let mask = 1 << shift;

        let mut bits = Self::EMPTY.0;
        bits[index] = mask;

        Self(bits)
    }

    pub fn is_empty(&self) -> bool {
        self == &SyntaxSet::EMPTY
    }

    pub fn insert(&mut self, value: SyntaxKind) {
        let set = std::mem::take(self);
        *self = set.union(&SyntaxSet::single(value));
    }

    pub const fn intersection(mut self, other: &Self) -> Self {
        let mut index = 0;

        while index < self.0.len() {
            self.0[index] &= other.0[index];
            index += 1;
        }

        self
    }

    pub const fn union(mut self, other: &Self) -> Self {
        let mut i = 0;

        while i < self.0.len() {
            self.0[i] |= other.0[i];
            i += 1;
        }

        self
    }

    pub const fn intersects(&self, other: &Self) -> bool {
        let mut i = 0;

        while i < self.0.len() {
            if self.0[i] & other.0[i] != 0 {
                return true;
            }
            i += 1;
        }

        false
    }

    pub const fn contains(&self, kind: SyntaxKind) -> bool {
        let kind = kind as u16;
        let index = kind as usize / Self::SLICE_BITS as usize;
        let shift = kind % Self::SLICE_BITS;
        let mask = 1 << shift;

        self.0[index] & mask != 0
    }

    pub const fn len(&self) -> usize {
        let mut len: u32 = 0;

        let mut index = 0;
        while index < self.0.len() {
            len += self.0[index].count_ones();
            index += 1;
        }

        len as usize
    }

    pub fn iter(&self) -> SyntaxSetIter {
        SyntaxSetIter {
            set: self.clone(),
            index: 0,
        }
    }
}

impl Extend<SyntaxKind> for SyntaxSet {
    fn extend<T: IntoIterator<Item = SyntaxKind>>(&mut self, iter: T) {
        let set = std::mem::take(self);
        *self = set.union(&SyntaxSet::from_iter(iter));
    }
}

impl IntoIterator for SyntaxSet {
    type Item = SyntaxKind;
    type IntoIter = SyntaxSetIter;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl IntoIterator for &SyntaxSet {
    type Item = SyntaxKind;
    type IntoIter = SyntaxSetIter;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl FromIterator<SyntaxKind> for SyntaxSet {
    fn from_iter<T: IntoIterator<Item = SyntaxKind>>(iter: T) -> Self {
        let mut set = SyntaxSet::EMPTY;

        for kind in iter {
            set.insert(kind);
        }

        set
    }
}

pub struct SyntaxSetIter {
    set: SyntaxSet,
    index: u16,
}

impl Iterator for SyntaxSetIter {
    type Item = SyntaxKind;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let slice = self.set.0.get_mut(self.index as usize)?;
            let bit = slice.trailing_zeros() as u16;

            if bit < SyntaxSet::SLICE_BITS {
                *slice ^= 1 << bit;
                let value = self.index * SyntaxSet::SLICE_BITS + bit;
                return Some(unsafe { std::mem::transmute::<u16, SyntaxKind>(value) });
            }

            self.index += 1;
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.set.len();

        (len, Some(len))
    }
}
